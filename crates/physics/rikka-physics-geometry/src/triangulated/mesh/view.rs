use rikka_physics_math::{Aabb, Vec3};

use super::data::MeshSlices;
use crate::triangulated::bvh::DecodedTree;
use crate::triangulated::sweep::TriangleSource;
use crate::triangulated::triangle::{Triangle, triangles_overlap_box};

#[derive(Clone, Copy, Debug)]
pub(crate) struct MeshView<'a> {
    slices: MeshSlices<'a>,
    tree: DecodedTree<'a>,
    scale: Vec3,
    mirrored: bool,
}

#[inline]
pub(crate) fn scale_bounds(bounds: &Aabb, scale: Vec3) -> Aabb {
    let a = bounds.min * scale;
    let b = bounds.max * scale;
    Aabb::new(a.min(b), a.max(b))
}

impl<'a> MeshView<'a> {
    #[inline]
    pub(crate) fn new(slices: MeshSlices<'a>, tree: DecodedTree<'a>, scale: Vec3) -> Self {
        Self {
            slices,
            tree,
            scale,
            mirrored: scale.x * scale.y * scale.z < 0.0,
        }
    }

    #[inline]
    pub(crate) fn triangle(&self, index: u32) -> Triangle {
        let mut triangle = self.slices.triangle(index);
        let scale = self.scale;
        triangle = triangle.transformed(|vertex| vertex * scale);
        if self.mirrored {
            triangle.vertices.swap(1, 2);
            triangle.indices.swap(1, 2);
            triangle.flags = triangle.flags.mirrored().inverted();
        }
        triangle
    }

    pub(crate) fn query_triangles(
        &self,
        bounds: Aabb,
        mut visit: impl FnMut(Vec3, Vec3, Vec3, u32) -> bool,
    ) -> bool {
        let scale = self.scale;
        self.for_each_overlapping(bounds, |index, [a, b, c]| {
            let (a, b, c) = (a * scale, b * scale, c * scale);
            if self.mirrored {
                visit(a, c, b, index)
            } else {
                visit(a, b, c, index)
            }
        })
    }

    fn for_each_overlapping(
        &self,
        bounds: Aabb,
        mut visit: impl FnMut(u32, [Vec3; 3]) -> bool,
    ) -> bool {
        let local = scale_bounds(&bounds, self.scale.recip());
        let center = local.center();
        let extent = local.extent();
        self.tree.query_leaves(local, |start, count| {
            let end = start + count;
            let mut chunk = start;
            while chunk < end {
                let last = end - 1;
                let triangles: [[Vec3; 3]; 4] = std::array::from_fn(|lane| {
                    self.slices.vertices((chunk + lane as u32).min(last))
                });
                let mut mask = triangles_overlap_box(center, extent, &triangles)
                    & ((1u32 << (end - chunk).min(4)) - 1);
                while mask != 0 {
                    let lane = mask.trailing_zeros();
                    mask &= mask - 1;
                    if !visit(chunk + lane, triangles[lane as usize]) {
                        return false;
                    }
                }
                chunk += 4;
            }
            true
        })
    }
}

impl TriangleSource for MeshView<'_> {
    fn triangles(&self) -> impl Fn(u32) -> Triangle + '_ {
        |index| self.triangle(index)
    }

    fn sweep_candidates(
        &self,
        origin: Vec3,
        translation: Vec3,
        extent: Vec3,
        max_fraction: f32,
        mut visit: impl FnMut(Triangle, f32) -> f32,
    ) -> f32 {
        let inverse = self.scale.recip();
        let local = Aabb::from_center_extent(origin * inverse, (extent * inverse).abs());
        self.tree
            .box_cast(local, translation * inverse, max_fraction, |index, best| {
                visit(self.triangle(index), best)
            })
    }

    fn overlap_candidates(&self, bounds: Aabb, mut visit: impl FnMut(Triangle) -> bool) -> bool {
        self.for_each_overlapping(bounds, |index, _| visit(self.triangle(index)))
    }

    fn overlapping(&self, bounds: Aabb, mut visit: impl FnMut(u32) -> bool) -> bool {
        self.for_each_overlapping(bounds, |index, _| visit(index))
    }
}
