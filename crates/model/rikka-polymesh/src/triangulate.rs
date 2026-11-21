use std::cmp::Ordering;
use std::collections::BinaryHeap;

use glam::{Vec2, Vec3};
use rikka_model_core::polygon;
use rustc_hash::FxHashMap;

use crate::normals::polygon_normal;
use crate::polymesh::Polymesh;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Triangulation {
    pub triangles: Vec<[u32; 3]>,
    pub faces: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum QuadMethod {
    #[default]
    Beauty,
    Fixed,
    Alternate,
    ShortEdge,
    LongEdge,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum NgonMethod {
    #[default]
    Beauty,
    Clip,
}

impl Polymesh {
    #[must_use]
    pub fn triangulate(&self) -> Triangulation {
        let mut triangulation = Triangulation {
            triangles: Vec::with_capacity(
                self.corner_count().saturating_sub(self.face_count() * 2),
            ),
            faces: Vec::new(),
        };
        for face in 0..self.face_count() {
            let before = triangulation.triangles.len();
            self.triangulate_face(face, &mut triangulation.triangles);
            let added = triangulation.triangles.len() - before;
            triangulation
                .faces
                .extend(std::iter::repeat_n(face as u32, added));
        }
        triangulation
    }

    #[must_use]
    pub fn point_triangulation(&self) -> Triangulation {
        let mut triangulation = self.triangulate();
        for triangle in &mut triangulation.triangles {
            *triangle = triangle.map(|corner| self.corner_verts[corner as usize]);
        }
        triangulation
    }

    #[must_use]
    pub fn point_triangles(&self) -> Vec<[u32; 3]> {
        self.point_triangulation().triangles
    }

    pub fn triangulate_face(&self, face: usize, out: &mut Vec<[u32; 3]>) {
        let range = self.face(face);
        let corners = &self.corner_verts[range.clone()];
        let start = range.start as u32;
        let at = |slot: usize| self.positions[corners[slot] as usize];
        match corners.len() {
            0..=2 => {}
            3 => out.push([start, start + 1, start + 2]),
            4 => match quad_flips(at(0), at(1), at(2), at(3)) {
                false => {
                    out.push([start, start + 1, start + 2]);
                    out.push([start, start + 2, start + 3]);
                }
                true => {
                    out.push([start, start + 1, start + 3]);
                    out.push([start + 1, start + 2, start + 3]);
                }
            },
            _ => {
                let projected = project(&self.positions, corners);
                for triangle in polygon::triangulate_flat(&projected) {
                    out.push(triangle.map(|slot| start + slot));
                }
            }
        }
    }

    #[must_use]
    pub fn triangulate_face_with(
        &self,
        face: usize,
        quad: QuadMethod,
        ngon: NgonMethod,
    ) -> Vec<[u32; 3]> {
        let range = self.face(face);
        let start = range.start as u32;
        let corners = &self.corner_verts[range];
        match corners.len() {
            0..=2 => Vec::new(),
            3 => vec![[start, start + 1, start + 2]],
            4 => {
                let [v1, v2, v3, v4] =
                    [0, 1, 2, 3].map(|slot| self.positions[corners[slot] as usize]);
                let split_24 = match quad {
                    QuadMethod::Fixed => false,
                    QuadMethod::Alternate => true,
                    QuadMethod::ShortEdge => {
                        v3.distance_squared(v1) - v4.distance_squared(v2) > 0.0
                    }
                    QuadMethod::LongEdge => {
                        v3.distance_squared(v1) - v4.distance_squared(v2) <= 0.0
                    }
                    QuadMethod::Beauty => quad_beauty(v1, v2, v3, v4) > 0.0,
                };
                match split_24 {
                    true => vec![
                        [start, start + 1, start + 3],
                        [start + 1, start + 2, start + 3],
                    ],
                    false => vec![[start, start + 1, start + 2], [start, start + 2, start + 3]],
                }
            }
            _ => {
                let projected = project(&self.positions, corners);
                let mut triangles = polygon::triangulate_flat(&projected);
                if ngon == NgonMethod::Beauty {
                    beautify(&projected, &mut triangles);
                }
                triangles
                    .into_iter()
                    .map(|triangle| triangle.map(|slot| start + slot))
                    .collect()
            }
        }
    }
}

fn project(positions: &[Vec3], corners: &[u32]) -> Vec<Vec2> {
    polygon::project(
        corners.iter().map(|&point| positions[point as usize]),
        polygon_normal(positions, corners),
    )
}

fn quad_beauty(v1: Vec3, v2: Vec3, v3: Vec3, v4: Vec3) -> f32 {
    let normal = (v3 - v2).cross(v4 - v2) + (v4 - v2).cross(v1 - v2);
    let scale = normal.length();
    if scale == 0.0 {
        return f32::MAX;
    }
    let (first, second) = polygon::basis(normal / scale);
    let flat = |point: Vec3| Vec2::new(first.dot(point), second.dot(point));
    let [p1, p2, p3, p4] = [v1, v2, v3, v4].map(flat);
    let signum = |value: f32| match value {
        value if value > 1.0e-5 => 1,
        value if value < -1.0e-5 => -1,
        _ => 0,
    };
    if signum(cross(p2, p3, p4) / scale) + signum(cross(p2, p4, p1) / scale) == 0 {
        return f32::MAX;
    }
    rotate_cost(p1, p2, p3, p4, true).0
}

fn cross(a: Vec2, b: Vec2, c: Vec2) -> f32 {
    (a.x - b.x) * (b.y - c.y) + (a.y - b.y) * (c.x - b.x)
}

fn rotate_cost(v1: Vec2, v2: Vec2, v3: Vec2, v4: Vec2, lock_degenerate: bool) -> (f32, f32) {
    const ZERO_AREA: f32 = 1.0e-12;
    let area_234 = cross(v2, v3, v4);
    let area_241 = cross(v2, v4, v1);
    let area_123 = cross(v1, v2, v3);
    let area_134 = cross(v1, v3, v4);
    let area = (area_234.abs() + area_241.abs() + area_123.abs() + area_134.abs()) / 8.0;
    if (area_123 >= 0.0) != (area_134 >= 0.0)
        || area_123.abs() <= ZERO_AREA
        || area_134.abs() <= ZERO_AREA
    {
        return (f32::MAX, area);
    }
    if (area_234 >= 0.0) != (area_241 >= 0.0) {
        return match lock_degenerate {
            true => (f32::MAX, area),
            false => (-f32::MAX, area),
        };
    }
    if area_234.abs() <= ZERO_AREA || area_241.abs() <= ZERO_AREA {
        return (-f32::MAX, area);
    }
    let len_12 = v1.distance(v2);
    let len_23 = v2.distance(v3);
    let len_34 = v3.distance(v4);
    let len_41 = v4.distance(v1);
    let len_13 = v1.distance(v3);
    let len_24 = v2.distance(v4);
    let fac_24 =
        area_234.abs() / (len_23 + len_34 + len_24) + area_241.abs() / (len_41 + len_12 + len_24);
    let fac_13 =
        area_123.abs() / (len_12 + len_23 + len_13) + area_134.abs() / (len_34 + len_41 + len_13);
    (fac_24 - fac_13, area)
}

#[derive(Clone, Copy)]
struct HalfEdge {
    vertex: u32,
    next: usize,
    radial: usize,
    base: usize,
}

struct Candidate {
    cost: f32,
    edge: usize,
    version: u32,
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Candidate {}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .total_cmp(&self.cost)
            .then_with(|| other.edge.cmp(&self.edge))
    }
}

fn beautify(coords: &[Vec2], triangles: &mut [[u32; 3]]) {
    let last = coords.len() as u32 - 1;
    let mut edges: Vec<HalfEdge> = Vec::with_capacity(triangles.len() * 3);
    let mut pairs: FxHashMap<[u32; 2], usize> = FxHashMap::default();
    let mut bases: Vec<usize> = Vec::new();
    for (triangle_index, triangle) in triangles.iter().enumerate() {
        for (previous, &vertex) in triangle.iter().enumerate() {
            let current = (previous + 1) % 3;
            edges.push(HalfEdge {
                vertex,
                next: triangle_index * 3 + current,
                radial: usize::MAX,
                base: usize::MAX,
            });
        }
    }
    for index in 0..edges.len() {
        let a = edges[index].vertex;
        let b = edges[edges[index].next].vertex;
        let key = match a < b {
            true => [a, b],
            false => [b, a],
        };
        let boundary = key[1] - key[0] == 1 || (key[0] == 0 && key[1] == last);
        if boundary {
            continue;
        }
        match pairs.remove(&key) {
            Some(other) => {
                edges[index].radial = other;
                edges[other].radial = index;
                edges[index].base = bases.len();
                edges[other].base = bases.len();
                bases.push(index);
            }
            None => {
                pairs.insert(key, index);
            }
        }
    }

    let cost_of = |edges: &[HalfEdge], edge: usize| -> (f32, f32) {
        let radial = edges[edge].radial;
        let other_a = edges[edges[edges[edge].next].next].vertex;
        let other_b = edges[edges[edges[radial].next].next].vertex;
        rotate_cost(
            coords[other_a as usize],
            coords[edges[edge].vertex as usize],
            coords[other_b as usize],
            coords[edges[radial].vertex as usize],
            false,
        )
    };
    let mut versions = vec![0u32; bases.len()];
    let mut queued = vec![false; bases.len()];
    let mut heap = BinaryHeap::new();
    let update = |edges: &[HalfEdge],
                  edge: usize,
                  heap: &mut BinaryHeap<Candidate>,
                  versions: &mut [u32],
                  queued: &mut [bool]| {
        let base = edges[edge].base;
        if base == usize::MAX {
            return;
        }
        let (cost, area) = cost_of(edges, edge);
        versions[base] += 1;
        queued[base] = cost < -1.0e-6 * area.max(1.0);
        if queued[base] {
            heap.push(Candidate {
                cost,
                edge,
                version: versions[base],
            });
        }
    };
    for &edge in &bases {
        update(&edges, edge, &mut heap, &mut versions, &mut queued);
    }
    let mut guard = bases.len() * bases.len() + 16;
    while let Some(candidate) = heap.pop() {
        let base = edges[candidate.edge].base;
        if base == usize::MAX || !queued[base] || versions[base] != candidate.version {
            continue;
        }
        if guard == 0 {
            break;
        }
        guard -= 1;
        queued[base] = false;
        let edge = candidate.edge;
        let e1 = edges[edge].next;
        let e2 = edges[e1].next;
        let e3 = edges[edge].radial;
        let e4 = edges[e3].next;
        let e5 = edges[e4].next;
        edges[edge].next = e2;
        edges[e1].next = e3;
        edges[e2].next = e4;
        edges[e3].next = e5;
        edges[e4].next = edge;
        edges[e5].next = e1;
        edges[edge].vertex = edges[e5].vertex;
        edges[e3].vertex = edges[e2].vertex;
        for neighbour in [
            edges[edge].next,
            edges[edges[edge].next].next,
            edges[e3].next,
            edges[edges[e3].next].next,
        ] {
            update(&edges, neighbour, &mut heap, &mut versions, &mut queued);
        }
    }

    let mut seen = vec![false; edges.len()];
    let mut written = 0;
    for start in 0..edges.len() {
        if seen[start] || written >= triangles.len() {
            continue;
        }
        let second = edges[start].next;
        let third = edges[second].next;
        seen[start] = true;
        seen[second] = true;
        seen[third] = true;
        triangles[written] = [
            edges[start].vertex,
            edges[second].vertex,
            edges[third].vertex,
        ];
        written += 1;
    }
}

fn quad_flips(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> bool {
    let along_ab = b - a;
    let along_ac = c - a;
    let along_ad = d - a;
    along_ab.cross(along_ac).dot(along_ad.cross(along_ac)) > 0.0
}
