use bytemuck::{Pod, Zeroable};
use rikka_physics_math::{F32x4, Float, Plane, Vec3, Vector};

use crate::convex::extrema::{larger, smaller};

pub const SOA_ALIGNMENT: usize = 8;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct HullEdge {
    pub next: u16,
    pub twin: u16,
    pub face: u16,
    pub origin: u8,
    pub padding: u8,
}

impl HullEdge {
    #[inline]
    pub const fn new(next: u16, twin: u16, face: u16, origin: u8) -> Self {
        Self {
            next,
            twin,
            face,
            origin,
            padding: 0,
        }
    }
}

#[inline]
pub(crate) const fn soa_stride(count: usize) -> usize {
    count.next_multiple_of(SOA_ALIGNMENT)
}

#[inline]
pub(crate) fn rows<const N: usize>(values: &[f32], stride: usize) -> [&[f32]; N] {
    std::array::from_fn(|row| &values[row * stride..(row + 1) * stride])
}

#[inline]
fn max_dot(soa: [&[f32]; 3], direction: Vec3) -> f32 {
    let [xs, ys, zs] = soa.map(|row| row.as_chunks::<4>().0);
    let dx = F32x4::splat(direction.x);
    let dy = F32x4::splat(direction.y);
    let dz = F32x4::splat(direction.z);
    let mut best = F32x4::splat(-f32::MAX);
    for ((x, y), z) in xs.iter().zip(ys).zip(zs) {
        let dot =
            dx * F32x4::from_array(*x) + dy * F32x4::from_array(*y) + dz * F32x4::from_array(*z);
        best = best.max(dot);
    }
    let [d0, d1, d2, d3] = best.to_array();
    larger(larger(d0, d1), larger(d2, d3))
}

#[inline]
pub(crate) fn horizontal_argmax(best: F32x4, best_index: F32x4) -> usize {
    let dots = best.to_array::<4>();
    let top = larger(larger(dots[0], dots[1]), larger(dots[2], dots[3]));
    let winners = F32x4::select(
        best.simd_eq(F32x4::splat(top)),
        best_index,
        F32x4::splat(f32::MAX),
    );
    let [i0, i1, i2, i3] = winners.to_array();
    smaller(smaller(i0, i1), smaller(i2, i3)) as usize
}

#[inline]
pub(crate) fn argmax(soa: [&[f32]; 3], direction: Vec3) -> usize {
    let [xs, ys, zs] = soa.map(|row| row.as_chunks::<4>().0);
    let dx = F32x4::splat(direction.x);
    let dy = F32x4::splat(direction.y);
    let dz = F32x4::splat(direction.z);
    let step = F32x4::splat(F32x4::LANES as f32);
    let mut index = F32x4::from_array([0.0, 1.0, 2.0, 3.0]);
    let mut best = F32x4::splat(-f32::MAX);
    let mut best_index = F32x4::default();
    for ((x, y), z) in xs.iter().zip(ys).zip(zs) {
        let dot =
            dx * F32x4::from_array(*x) + dy * F32x4::from_array(*y) + dz * F32x4::from_array(*z);
        let better = dot.simd_gt(best);
        best = F32x4::select(better, dot, best);
        best_index = F32x4::select(better, index, best_index);
        index += step;
    }
    horizontal_argmax(best, best_index)
}

pub(crate) const EDGE_ROWS: usize = 8;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Polyhedron<'a> {
    pub(crate) points: &'a [Vec3],
    pub(crate) vertex_edges: &'a [u16],
    pub(crate) edges: &'a [HullEdge],
    pub(crate) faces: &'a [u16],
    pub(crate) planes: &'a [Plane],
    pub(crate) soa_points: &'a [f32],
    pub(crate) soa_normals: &'a [f32],
    pub(crate) edge_normals: &'a [f32],
    pub(crate) face_distances: &'a [f32],
    pub(crate) center: Vec3,
    pub(crate) inner_radius: f32,
    pub(crate) box_axes: [Vec3; 3],
}

impl<'a> Polyhedron<'a> {
    #[inline]
    pub(crate) fn vertex_count(&self) -> usize {
        self.points.len()
    }

    #[inline]
    pub(crate) fn face_count(&self) -> usize {
        self.planes.len()
    }

    #[inline]
    pub(crate) fn edge_pair_count(&self) -> usize {
        self.edges.len() / 2
    }

    #[inline]
    pub(crate) fn soa_vertices(&self) -> [&'a [f32]; 3] {
        rows(self.soa_points, soa_stride(self.vertex_count()))
    }

    #[inline]
    pub(crate) fn soa_face_normals(&self) -> [&'a [f32]; 3] {
        rows(self.soa_normals, soa_stride(self.face_count()))
    }

    #[inline]
    pub(crate) fn edge_rows(&self) -> [&'a [f32]; EDGE_ROWS] {
        rows(self.edge_normals, soa_stride(self.edge_pair_count()))
    }

    #[inline]
    pub(crate) fn edge_points(&self, edge: usize) -> (Vec3, Vec3) {
        let half = self.edges[edge];
        let twin = self.edges[half.twin as usize];
        (
            self.points[half.origin as usize],
            self.points[twin.origin as usize],
        )
    }

    pub(crate) fn face_edges(&self, face: usize) -> FaceEdges<'a> {
        let first = self.faces[face];
        FaceEdges {
            edges: self.edges,
            first,
            current: Some(first),
        }
    }

    #[inline]
    pub(crate) fn support_vertex(&self, direction: Vec3) -> usize {
        argmax(self.soa_vertices(), direction).min(self.vertex_count() - 1)
    }

    #[inline]
    pub(crate) fn max_dot(&self, direction: Vec3) -> f32 {
        max_dot(self.soa_vertices(), direction)
    }

    #[inline]
    pub(crate) fn aligned_face(&self, direction: Vec3) -> usize {
        argmax(self.soa_face_normals(), direction).min(self.face_count() - 1)
    }

    pub(crate) fn support_face(&self, direction: Vec3) -> usize {
        let mut best = 0;
        let mut best_dot = -f32::MAX;
        for (index, plane) in self.planes.iter().enumerate() {
            let dot = plane.normal.dot(direction);
            if dot > best_dot {
                best = index;
                best_dot = dot;
            }
        }
        best
    }

    pub(crate) fn incident_face(&self, reference_normal: Vec3, vertex: usize) -> usize {
        let start = self.vertex_edges[vertex] as usize;
        let origin = self.points[vertex];
        let mut edge = start;
        let mut best_edge = start;
        let mut best_projection = f32::MAX;
        let mut best_length = 1.0;
        loop {
            let twin = self.edges[edge].twin as usize;
            let axis = self.points[self.edges[twin].origin as usize] - origin;
            let projection = axis.dot(reference_normal);
            let projection = projection * projection;
            let length = axis.length_squared();
            if projection * best_length < best_projection * length {
                best_edge = edge;
                best_projection = projection;
                best_length = length;
            }
            edge = self.edges[twin].next as usize;
            if edge == start {
                break;
            }
        }
        let face1 = self.edges[best_edge].face as usize;
        let face2 = self.edges[self.edges[best_edge].twin as usize].face as usize;
        if self.planes[face1].normal.dot(reference_normal)
            < self.planes[face2].normal.dot(reference_normal)
        {
            face1
        } else {
            face2
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FaceEdges<'a> {
    edges: &'a [HullEdge],
    first: u16,
    current: Option<u16>,
}

impl Iterator for FaceEdges<'_> {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<usize> {
        let current = self.current?;
        let next = self.edges[current as usize].next;
        self.current = (next != self.first).then_some(next);
        Some(current as usize)
    }
}
