use rikka_physics_math::{Plane, Transform, Vec3};

use super::clip::{
    CONVEX, ClipBuffers, ClipPoint, Pair, TRIANGLE, line_closest, plane_from_points,
};
use super::hull_table::{EdgeGeometry, HullTable};
use super::hull_view::HullView;
use super::local::{ContactFeature, LocalManifold, PointSink};
use super::triangle_capsule::triangle_feature;
use crate::triangulated::triangle::TriangleFeature;
use crate::{DistanceInput, ShapeProxy, SimplexCache, shape_distance};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum AxisKind {
    #[default]
    Invalid,
    Backside,
    TriangleFace,
    HullFace,
    EdgePair,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct SatCache {
    pub(crate) kind: AxisKind,
    pub(crate) index_a: u8,
    pub(crate) index_b: u16,
    pub(crate) separation: f32,
}

#[derive(Clone, Copy, Debug)]
struct Axis {
    normal: Vec3,
    separation: f32,
    index_a: usize,
    index_b: usize,
}

struct TriangleData {
    points: [Vec3; 3],
    edges: [Vec3; 3],
    plane: Plane,
}

pub(crate) struct HullContact<'a, 'b> {
    pub(crate) hull: &'a HullView<'a>,
    pub(crate) speculative: f32,
    pub(crate) linear_slop: f32,
    pub(crate) sink: &'a mut PointSink<'b>,
    pub(crate) clip: &'a mut ClipBuffers,
    pub(crate) table: &'a mut HullTable,
}

fn query_triangle_face(
    triangle: &TriangleData,
    hull: &HullView<'_>,
    table: &mut HullTable,
) -> Axis {
    let vertex = table.support(hull, -triangle.plane.normal);
    Axis {
        normal: triangle.plane.normal,
        separation: triangle.plane.separation(hull.points[vertex]),
        index_a: 0,
        index_b: vertex,
    }
}

#[inline]
fn triangle_support(points: &[Vec3; 3], direction: Vec3) -> usize {
    let mut best = 0;
    let mut value = points[0].dot(direction);
    for (index, point) in points.iter().enumerate().skip(1) {
        let d = point.dot(direction);
        if d > value {
            value = d;
            best = index;
        }
    }
    best
}

fn query_hull_face(triangle: &TriangleData, hull: &HullView<'_>, table: &mut HullTable) -> Axis {
    let face = table.deepest_face(hull, &triangle.points);
    Axis {
        normal: -hull.planes[face.face].normal,
        separation: face.separation,
        index_a: face.vertex,
        index_b: face.face,
    }
}

fn query_edges(
    triangle: &TriangleData,
    hull: &HullView<'_>,
    table: &mut HullTable,
) -> Option<Axis> {
    table
        .best_edge_pair(
            hull,
            &triangle.points,
            &triangle.edges,
            triangle.plane.normal,
        )
        .map(|pair| Axis {
            normal: -pair.axis,
            separation: pair.separation,
            index_a: pair.triangle_edge,
            index_b: pair.hull_edge,
        })
}

#[inline]
fn emit(
    sink: &mut PointSink<'_>,
    radius: f32,
    triangle_point: Vec3,
    hull_point: Vec3,
    normal: Vec3,
    separation: f32,
    id: u32,
) {
    sink.push(
        0.5 * (triangle_point + hull_point - radius * normal),
        separation - radius,
        id,
    );
}

impl HullContact<'_, '_> {
    #[inline]
    fn push(
        &mut self,
        triangle_point: Vec3,
        hull_point: Vec3,
        normal: Vec3,
        separation: f32,
        id: u32,
    ) {
        emit(
            self.sink,
            self.hull.radius,
            triangle_point,
            hull_point,
            normal,
            separation,
            id,
        );
    }

    fn hull_face(
        &mut self,
        triangle: &TriangleData,
        axis: Axis,
        cache: &mut SatCache,
    ) -> (Option<LocalManifold>, f32) {
        let reference = self.hull.planes[axis.index_b];
        let radius = self.hull.radius;
        let (mut input, mut output) = self.clip.pair();
        input.count = 0;
        for (index, &position) in triangle.points.iter().enumerate() {
            input.push(ClipPoint {
                position,
                separation: reference.separation(position),
                pair: Pair::new(TRIANGLE, ((index + 2) % 3) as u16, TRIANGLE, index as u16),
            });
        }
        for edge in self.hull.face_edges(axis.index_b) {
            let (vertex1, vertex2) = (
                self.hull.points[self.hull.edges[edge].origin as usize],
                self.hull.points
                    [self.hull.edges[self.hull.edges[edge].next as usize].origin as usize],
            );
            let binormal = (vertex2 - vertex1)
                .normalize_or_zero()
                .cross(reference.normal);
            input.clip(
                output,
                Plane::from_normal_point(binormal, vertex1),
                edge as u16,
                CONVEX,
                reference,
            );
            if output.count < 3 {
                *cache = SatCache::default();
                return (None, axis.separation);
            }
            std::mem::swap(&mut input, &mut output);
        }
        let min_separation = input
            .points()
            .iter()
            .fold(f32::MAX, |min, point| min.min(point.separation));
        if min_separation > self.speculative {
            *cache = SatCache::default();
            return (None, min_separation);
        }
        for point in input.points() {
            emit(
                self.sink,
                radius,
                point.position,
                point.position - point.separation * reference.normal,
                -reference.normal,
                point.separation,
                point.pair.id(),
            );
        }
        *cache = SatCache {
            kind: AxisKind::HullFace,
            index_a: axis.index_a as u8,
            index_b: axis.index_b as u16,
            separation: min_separation,
        };
        (
            Some(LocalManifold {
                normal: -reference.normal,
                feature: ContactFeature::HullFace,
                squared_distance: 0.0,
            }),
            min_separation,
        )
    }

    fn triangle_face(
        &mut self,
        triangle: &TriangleData,
        axis: Axis,
        cache: &mut SatCache,
    ) -> (Option<LocalManifold>, f32) {
        let reference = triangle.plane;
        let face = self.hull.incident_face(reference.normal, axis.index_b);
        let radius = self.hull.radius;
        let (mut input, mut output) = self.clip.pair();
        input.count = 0;
        for edge in self.hull.face_edges(face) {
            let next = self.hull.edges[edge].next as usize;
            let position = self.hull.points[self.hull.edges[next].origin as usize];
            input.push(ClipPoint {
                position,
                separation: reference.separation(position),
                pair: Pair::new(CONVEX, edge as u16, CONVEX, next as u16),
            });
        }
        for index in 0..3 {
            if input.count == 0 {
                break;
            }
            let side = triangle.edges[index]
                .cross(reference.normal)
                .normalize_or_zero();
            input.clip(
                output,
                Plane::from_normal_point(side, triangle.points[index]),
                index as u16,
                TRIANGLE,
                reference,
            );
            std::mem::swap(&mut input, &mut output);
        }
        if input.count == 0 {
            *cache = SatCache::default();
            return (None, f32::MAX);
        }
        let min_separation = input
            .points()
            .iter()
            .fold(f32::MAX, |min, point| min.min(point.separation));
        if min_separation >= self.speculative {
            *cache = SatCache::default();
            return (None, min_separation);
        }
        for point in input.points() {
            emit(
                self.sink,
                radius,
                point.position - point.separation * reference.normal,
                point.position,
                reference.normal,
                point.separation,
                point.pair.id(),
            );
        }
        *cache = SatCache {
            kind: AxisKind::TriangleFace,
            index_a: 0,
            index_b: axis.index_b as u16,
            separation: min_separation,
        };
        (
            Some(LocalManifold {
                normal: reference.normal,
                feature: ContactFeature::Triangle(TriangleFeature::Face),
                squared_distance: 0.0,
            }),
            min_separation,
        )
    }

    fn edges(
        &mut self,
        triangle: &TriangleData,
        axis: Axis,
        cache: &mut SatCache,
    ) -> Option<LocalManifold> {
        let point_a = triangle.points[axis.index_a];
        let edge_a = triangle.edges[axis.index_a];
        let (point_b, end_b) = self.hull.edge_points(axis.index_b);
        let (closest_a, fraction_a, closest_b, fraction_b) =
            line_closest(point_a, edge_a, point_b, end_b - point_b);
        if !(0.0..=1.0).contains(&fraction_a) || !(0.0..=1.0).contains(&fraction_b) {
            *cache = SatCache::default();
            return None;
        }
        let separation = axis.normal.dot(point_b - point_a);
        self.push(
            closest_a,
            closest_b,
            axis.normal,
            separation,
            Pair::new(TRIANGLE, axis.index_a as u16, CONVEX, axis.index_b as u16).id(),
        );
        *cache = SatCache {
            kind: AxisKind::EdgePair,
            index_a: axis.index_a as u8,
            index_b: axis.index_b as u16,
            separation,
        };
        Some(LocalManifold {
            normal: axis.normal,
            feature: ContactFeature::Triangle(TriangleFeature::Edge(axis.index_a as u8)),
            squared_distance: 0.0,
        })
    }

    fn cached(
        &mut self,
        triangle: &TriangleData,
        cache: &mut SatCache,
    ) -> Option<Option<LocalManifold>> {
        let slop = self.linear_slop;
        match cache.kind {
            AxisKind::TriangleFace => {
                let axis = query_triangle_face(triangle, self.hull, self.table);
                if axis.separation > self.speculative {
                    return Some(None);
                }
                let mut local = *cache;
                let (manifold, clipped) = self.triangle_face(triangle, axis, &mut local);
                if manifold.is_some() && (cache.separation - clipped).abs() < slop {
                    return Some(manifold);
                }
                self.sink.clear();
            }
            AxisKind::HullFace => {
                let face = cache.index_b as usize;
                if face >= self.hull.planes.len() {
                    *cache = SatCache::default();
                    return None;
                }
                let plane = self.hull.planes[face];
                let vertex = triangle_support(&triangle.points, -plane.normal);
                let separation = plane.separation(triangle.points[vertex]);
                if separation > self.speculative {
                    return Some(None);
                }
                if separation >= -2.0 * slop {
                    let axis = Axis {
                        normal: -plane.normal,
                        separation,
                        index_a: vertex,
                        index_b: face,
                    };
                    let mut local = *cache;
                    let (manifold, clipped) = self.hull_face(triangle, axis, &mut local);
                    if manifold.is_some() && (cache.separation - clipped).abs() < slop {
                        return Some(manifold);
                    }
                    self.sink.clear();
                }
            }
            AxisKind::EdgePair => {
                let index_a = cache.index_a as usize;
                let index_b = cache.index_b as usize;
                if index_a < 3 && index_b + 1 < self.hull.edges.len() {
                    let edge = triangle.edges[index_a];
                    let geometry = EdgeGeometry::from_hull(self.hull, index_b);
                    if let Some(axis) = geometry.axis(triangle.plane.normal, edge) {
                        let separation = axis.dot(triangle.points[index_a] - geometry.point);
                        if separation > self.speculative {
                            return Some(None);
                        }
                        if (cache.separation - separation).abs() < slop {
                            let query = Axis {
                                normal: -axis,
                                separation,
                                index_a,
                                index_b,
                            };
                            let mut local = *cache;
                            if let Some(manifold) = self.edges(triangle, query, &mut local) {
                                return Some(Some(manifold));
                            }
                        }
                    }
                }
            }
            AxisKind::Invalid | AxisKind::Backside => return None,
        }
        *cache = SatCache::default();
        None
    }

    pub(crate) fn collide(
        &mut self,
        points: &[Vec3; 3],
        cache: &mut SatCache,
    ) -> Option<LocalManifold> {
        let [v1, v2, v3] = *points;
        let plane = plane_from_points(v1, v2, v3);
        let slop = self.linear_slop;
        let offset = plane.separation(self.hull.center);
        if cache.kind == AxisKind::Backside {
            if (cache.separation - offset).abs() < slop {
                return None;
            }
            cache.kind = AxisKind::Invalid;
        }
        if offset < -slop {
            *cache = SatCache {
                kind: AxisKind::Backside,
                separation: offset,
                ..SatCache::default()
            };
            return None;
        }
        let triangle = TriangleData {
            points: *points,
            edges: [v2 - v1, v3 - v2, v1 - v3],
            plane,
        };
        if let Some(result) = self.cached(&triangle, cache) {
            return result;
        }
        let speculative = self.speculative;
        let face_a = query_triangle_face(&triangle, self.hull, self.table);
        if face_a.separation > speculative {
            *cache = SatCache {
                kind: AxisKind::TriangleFace,
                index_a: 0,
                index_b: face_a.index_b as u16,
                separation: face_a.separation,
            };
            return None;
        }
        let face_b = query_hull_face(&triangle, self.hull, self.table);
        if face_b.separation > speculative {
            *cache = SatCache {
                kind: AxisKind::HullFace,
                index_a: face_b.index_a as u8,
                index_b: face_b.index_b as u16,
                separation: face_b.separation,
            };
            return None;
        }
        let edge = query_edges(&triangle, self.hull, self.table);
        if let Some(edge) = edge
            && edge.separation > speculative
        {
            *cache = SatCache {
                kind: AxisKind::EdgePair,
                index_a: edge.index_a as u8,
                index_b: edge.index_b as u16,
                separation: edge.separation,
            };
            return None;
        }
        let pushing_down = face_b.normal.dot(plane.normal) < -0.25;
        let (mut manifold, clipped) = if face_b.separation >= face_a.separation && !pushing_down {
            self.hull_face(&triangle, face_b, cache)
        } else {
            self.triangle_face(&triangle, face_a, cache)
        };
        if let Some(edge) = edge {
            let max_face = face_a.separation.max(face_b.separation);
            let count = self.sink.count();
            if (count == 0 && edge.separation > max_face)
                || (count == 1 && edge.separation > clipped + slop)
            {
                self.sink.clear();
                manifold = self.edges(&triangle, edge, cache);
            }
        }
        if self.sink.count() == 0 {
            manifold = self.closest_points(points, cache);
        }
        manifold
    }

    fn closest_points(
        &mut self,
        points: &[Vec3; 3],
        cache: &mut SatCache,
    ) -> Option<LocalManifold> {
        *cache = SatCache::default();
        let triangle_proxy = ShapeProxy::new(points, 0.0);
        let hull_proxy = ShapeProxy::new(self.hull.points, 0.0);
        let mut simplex = SimplexCache::EMPTY;
        let input = DistanceInput::new(&triangle_proxy, &hull_proxy, Transform::IDENTITY);
        let output = shape_distance(&input, &mut simplex);
        if output.distance <= 0.0 {
            return None;
        }
        self.push(
            output.point_a,
            output.point_b,
            output.normal,
            output.distance,
            Pair::SINGLE.id(),
        );
        Some(LocalManifold {
            normal: output.normal,
            feature: ContactFeature::Triangle(triangle_feature(&simplex)),
            squared_distance: output.distance * output.distance,
        })
    }
}
