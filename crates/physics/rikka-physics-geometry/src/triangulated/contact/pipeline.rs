use rikka_physics_math::{Mat3, Transform, Vec3};

use super::cache::CachedFeature;
use super::clip::ClipBuffers;
use super::cull::Culler;
use super::hull_table::HullTable;
use super::hull_view::HullView;
use super::key_set::KeySet;
use super::local::{ContactFeature, LocalManifold, LocalPoint, PlacedTriangle, PointSink};
use super::triangle_capsule::collide_triangle_capsule;
use super::triangle_hull::HullContact;
use super::triangle_sphere::collide_triangle_sphere;
use crate::triangulated::sweep::TriangleSource;
use crate::triangulated::triangle::{EdgeFlags, Triangle, TriangleFeature, map3};
use crate::{Capsule, Manifold, ManifoldPoint, PairCache};

const CLUSTER_COSINE: f32 = 0.996;

#[derive(Clone, Copy, Debug)]
pub(crate) enum ConvexKind<'a> {
    Sphere { center: Vec3, radius: f32 },
    Capsule(Capsule),
    Hull(HullView<'a>),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Settings {
    pub(crate) speculative: f32,
    pub(crate) linear_slop: f32,
    pub(crate) rest_offset: f32,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Target<'a, S> {
    pub(crate) source: &'a S,
    pub(crate) salt: u32,
    pub(crate) material_base: u32,
    pub(crate) transform: Transform,
}
impl<'a, S> Target<'a, S> {
    #[inline]
    pub(crate) fn whole(source: &'a S, transform: Transform) -> Self {
        Self {
            source,
            salt: 0,
            material_base: 0,
            transform,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Hit {
    manifold: LocalManifold,
    start: usize,
    count: usize,
    triangle: Triangle,
    triangle_normal: Vec3,
}

#[derive(Clone, Copy, Debug)]
struct Cluster {
    normal: Vec3,
    triangle_normal: Vec3,
    material: u32,
}

#[derive(Debug, Default)]
pub(crate) struct Work {
    points: Vec<LocalPoint>,
    hits: Vec<Hit>,
    accepted: Vec<usize>,
    tentative: Vec<usize>,
    edges: KeySet,
    vertices: KeySet,
    clusters: Vec<Cluster>,
    membership: Vec<usize>,
    cluster_points: Vec<ManifoldPoint>,
    culler: Culler,
    clip: ClipBuffers,
    table: HullTable,
}

#[inline]
fn edge_key(a: u32, b: u32) -> u64 {
    u64::from(a.min(b)) << 32 | u64::from(a.max(b))
}

impl Work {
    fn add_triangle(&mut self, triangle: &Triangle) -> [bool; 6] {
        let [i1, i2, i3] = triangle.indices;
        [
            self.edges.insert(edge_key(i1, i2)),
            self.edges.insert(edge_key(i2, i3)),
            self.edges.insert(edge_key(i3, i1)),
            self.vertices.insert(u64::from(i1)),
            self.vertices.insert(u64::from(i2)),
            self.vertices.insert(u64::from(i3)),
        ]
    }

    fn has_edge(&self, triangle: &Triangle, edge: usize) -> bool {
        let indices = triangle.indices;
        self.edges
            .contains(edge_key(indices[edge], indices[(edge + 1) % 3]))
    }

    fn min_separation(&self, hit: &Hit) -> f32 {
        self.points[hit.start..hit.start + hit.count]
            .iter()
            .fold(f32::MAX, |min, point| min.min(point.separation))
    }

    pub(crate) fn collide(
        &mut self,
        target: &Target<'_, impl TriangleSource>,
        convex: &ConvexKind<'_>,
        settings: Settings,
        entries: &mut [CachedFeature],
        pairs: &mut [PairCache],
        out: &mut Vec<Manifold>,
    ) {
        self.points.clear();
        self.hits.clear();
        self.table.invalidate();
        let fetch = target.source.triangles();
        let rotation = Mat3::from_quat(target.transform.q);
        let translation = target.transform.p;
        for (index, entry) in entries.iter_mut().enumerate() {
            let triangle = fetch(entry.key.triangle);
            let vertices = map3(triangle.vertices, |vertex| rotation * vertex + translation);
            let start = self.points.len();
            let mut sink = PointSink::new(&mut self.points);
            let manifold = match convex {
                ConvexKind::Sphere { center, radius } => collide_triangle_sphere(
                    &vertices,
                    *center,
                    *radius,
                    settings.speculative,
                    &mut sink,
                ),
                ConvexKind::Capsule(capsule) => collide_triangle_capsule(
                    &PlacedTriangle {
                        local: triangle.vertices,
                        vertices,
                        transform: target.transform,
                    },
                    capsule,
                    settings.speculative,
                    settings.linear_slop,
                    &mut pairs[index].simplex,
                    &mut sink,
                ),
                ConvexKind::Hull(hull) => HullContact {
                    hull,
                    speculative: settings.speculative + hull.radius,
                    linear_slop: settings.linear_slop,
                    sink: &mut sink,
                    clip: &mut self.clip,
                    table: &mut self.table,
                }
                .collide(&vertices, &mut entry.sat),
            };
            let count = sink.count();
            match manifold {
                Some(manifold) if count > 0 => {
                    let [a, b, c] = vertices;
                    self.hits.push(Hit {
                        manifold,
                        start,
                        count,
                        triangle,
                        triangle_normal: (b - a).cross(c - a).normalize_or_zero(),
                    });
                }
                _ => sink.clear(),
            }
        }
        if self.hits.is_empty() {
            return;
        }
        self.filter_ghosts(
            matches!(convex, ConvexKind::Sphere { .. }),
            settings.linear_slop,
        );
        self.cluster(target.material_base);
        self.emit(target.salt, settings, out);
    }

    fn filter_ghosts(&mut self, sphere: bool, linear_slop: f32) {
        self.accepted.clear();
        self.tentative.clear();
        self.edges.reset(3 * self.hits.len());
        self.vertices.reset(3 * self.hits.len());
        for index in 0..self.hits.len() {
            let hit = self.hits[index];
            let accept = match hit.manifold.feature {
                ContactFeature::Triangle(TriangleFeature::Face) => true,
                ContactFeature::HullFace => {
                    hit.triangle_normal.dot(hit.manifold.normal) > 0.5
                        || self.min_separation(&hit) < -2.0 * linear_slop
                }
                ContactFeature::Triangle(_) => false,
            };
            if accept {
                self.add_triangle(&hit.triangle);
                self.accepted.push(index);
            } else {
                self.tentative.push(index);
            }
        }
        if sphere {
            let hits = &self.hits;
            self.tentative.sort_unstable_by(|&a, &b| {
                hits[a]
                    .manifold
                    .squared_distance
                    .total_cmp(&hits[b].manifold.squared_distance)
                    .then(a.cmp(&b))
            });
            for position in 0..self.tentative.len() {
                let index = self.tentative[position];
                let hit = self.hits[index];
                let added = self.add_triangle(&hit.triangle);
                let collide = match hit.manifold.feature {
                    ContactFeature::Triangle(TriangleFeature::Edge(edge)) => added[edge as usize],
                    ContactFeature::Triangle(TriangleFeature::Vertex(vertex)) => {
                        added[3 + vertex as usize]
                    }
                    _ => false,
                };
                if collide {
                    self.accepted.push(index);
                }
            }
        } else {
            for position in 0..self.tentative.len() {
                let index = self.tentative[position];
                let triangle = self.hits[index].triangle;
                let flags: EdgeFlags = triangle.flags;
                if flags.all_flat() {
                    continue;
                }
                let ghost =
                    (0..3).any(|edge| flags.is_flat(edge) && self.has_edge(&triangle, edge));
                if !ghost {
                    self.accepted.push(index);
                }
            }
        }
    }

    fn cluster(&mut self, material_base: u32) {
        self.clusters.clear();
        self.membership.clear();
        for &index in &self.accepted {
            let hit = &self.hits[index];
            let material = material_base + u32::from(hit.triangle.material);
            let found = self.clusters.iter().position(|cluster| {
                cluster.material == material
                    && cluster.normal.dot(hit.manifold.normal) > CLUSTER_COSINE
                    && cluster.triangle_normal.dot(hit.triangle_normal) > CLUSTER_COSINE
            });
            let cluster = found.unwrap_or_else(|| {
                self.clusters.push(Cluster {
                    normal: hit.manifold.normal,
                    triangle_normal: hit.triangle_normal,
                    material,
                });
                self.clusters.len() - 1
            });
            self.membership.push(cluster);
        }
    }

    fn emit(&mut self, salt: u32, settings: Settings, out: &mut Vec<Manifold>) {
        for cluster_index in 0..self.clusters.len() {
            let cluster = self.clusters[cluster_index];
            self.cluster_points.clear();
            for (&index, &member) in self.accepted.iter().zip(&self.membership) {
                if member != cluster_index {
                    continue;
                }
                let hit = &self.hits[index];
                for point in &self.points[hit.start..hit.start + hit.count] {
                    self.cluster_points.push(ManifoldPoint {
                        point: point.point,
                        separation: point.separation - settings.rest_offset,
                        feature: point.id ^ salt,
                        triangle: hit.triangle.index,
                    });
                }
            }
            self.culler.reduce(
                &mut self.cluster_points,
                cluster.triangle_normal,
                settings.linear_slop,
            );
            let mut manifold = Manifold {
                normal: -cluster.normal,
                material: cluster.material,
                ..Manifold::default()
            };
            for &point in self.cluster_points.iter().take(crate::MAX_MANIFOLD_POINTS) {
                manifold.push(point);
            }
            if !manifold.is_empty() {
                out.push(manifold);
            }
        }
    }
}
