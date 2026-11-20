use rikka_physics_math::{Aabb, Vec3};

use super::data::{HEIGHT_FIELD_HOLE, HeightFieldHeader, Level};
use super::tree::HeightFieldTree;
use crate::triangulated::bvh::{query_nodes, sweep_nodes};
use crate::triangulated::sweep::TriangleSource;
use crate::triangulated::triangle::{EdgeFlags, Triangle, triangle_overlaps_box};

#[derive(Clone, Copy, Debug)]
pub(crate) struct HeightFieldView<'a> {
    pub(crate) header: HeightFieldHeader,
    pub(crate) heights: &'a [u16],
    pub(crate) materials: &'a [u8],
    pub(crate) flags: &'a [u8],
    pub(crate) ranges: &'a [[u16; 2]],
    pub(crate) levels: &'a [Level],
}

#[inline]
fn cell_bounds(corners: &[Vec3; 4]) -> Aabb {
    Aabb::new(
        corners[0].min(corners[1]).min(corners[2]).min(corners[3]),
        corners[0].max(corners[1]).max(corners[2]).max(corners[3]),
    )
}

impl HeightFieldView<'_> {
    #[inline]
    pub(crate) fn scale(&self) -> Vec3 {
        self.header.scale
    }

    #[inline]
    pub(crate) fn columns(&self) -> u32 {
        self.header.columns
    }

    #[inline]
    pub(crate) fn rows(&self) -> u32 {
        self.header.rows
    }

    #[inline]
    pub(crate) fn tree(&self) -> HeightFieldTree<'_> {
        HeightFieldTree { field: self }
    }

    #[inline]
    pub(crate) fn is_hole(&self, cell: u32) -> bool {
        self.materials[cell as usize] == HEIGHT_FIELD_HOLE
    }

    #[inline]
    pub(crate) fn decode_height(&self, compressed: u16) -> f32 {
        let header = &self.header;
        header.scale.y * (header.min_height + header.height_scale * f32::from(compressed))
    }

    #[inline]
    pub(crate) fn height(&self, column: u32, row: u32) -> f32 {
        self.decode_height(self.heights[(row * self.columns() + column) as usize])
    }

    #[inline]
    fn vertex(&self, column: u32, row: u32) -> Vec3 {
        let scale = self.scale();
        Vec3::new(
            scale.x * column as f32,
            self.height(column, row),
            scale.z * row as f32,
        )
    }

    #[inline]
    fn cell_corners(&self, column: u32, row: u32) -> [Vec3; 4] {
        [
            self.vertex(column, row),
            self.vertex(column + 1, row),
            self.vertex(column, row + 1),
            self.vertex(column + 1, row + 1),
        ]
    }

    pub(crate) fn triangle(&self, index: u32) -> Triangle {
        let cell = index >> 1;
        let cells_x = self.columns() - 1;
        let row = cell / cells_x;
        let column = cell - row * cells_x;
        self.cell_triangle(column, row, index & 1, &self.cell_corners(column, row))
    }

    fn cell_triangle(&self, column: u32, row: u32, half: u32, corners: &[Vec3; 4]) -> Triangle {
        let columns = self.columns();
        let cell = row * (columns - 1) + column;
        let index = 2 * cell + half;
        let i11 = row * columns + column;
        let i12 = i11 + 1;
        let i21 = i11 + columns;
        let i22 = i21 + 1;
        let (vertices, indices) = if half == 0 {
            ([corners[0], corners[2], corners[1]], [i11, i21, i12])
        } else {
            ([corners[3], corners[1], corners[2]], [i22, i12, i21])
        };
        let mut triangle = Triangle {
            vertices,
            indices,
            flags: EdgeFlags(self.flags[index as usize]),
            material: self.materials[cell as usize],
            index,
        };
        if self.header.clockwise != 0 {
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
        self.for_each_triangle(bounds, |triangle| {
            let [a, b, c] = triangle.vertices;
            visit(a, b, c, triangle.index)
        })
    }

    fn for_each_triangle(&self, bounds: Aabb, mut visit: impl FnMut(Triangle) -> bool) -> bool {
        let tree = self.tree();
        let scale = self.scale();
        let center = bounds.center();
        let extent = bounds.extent();
        let lower = (bounds.min / scale).ceil() - Vec3::ONE;
        let upper = (bounds.max / scale).floor();
        query_nodes(&tree, &bounds, |block| {
            let (columns, rows) = tree.block_cells(block);
            let column_start = columns.start.max(lower.x.max(0.0) as u32);
            let column_end = columns.end.min((upper.x.max(-1.0) + 1.0) as u32);
            let row_start = rows.start.max(lower.z.max(0.0) as u32);
            let row_end = rows.end.min((upper.z.max(-1.0) + 1.0) as u32);
            for row in row_start..row_end {
                for column in column_start..column_end {
                    let cell = row * (self.columns() - 1) + column;
                    if self.is_hole(cell) {
                        continue;
                    }
                    let corners = self.cell_corners(column, row);
                    if !cell_bounds(&corners).overlaps(&bounds) {
                        continue;
                    }
                    for half in 0..2 {
                        let triangle = self.cell_triangle(column, row, half, &corners);
                        if triangle_overlaps_box(center, extent, triangle.vertices)
                            && !visit(triangle)
                        {
                            return false;
                        }
                    }
                }
            }
            true
        })
    }
}

impl TriangleSource for HeightFieldView<'_> {
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
        let tree = self.tree();
        sweep_nodes(
            &tree,
            origin,
            translation,
            extent,
            max_fraction,
            |block, best| {
                let mut best = best;
                let (columns, rows) = tree.block_cells(block);
                for row in rows {
                    for column in columns.clone() {
                        let cell = row * (self.columns() - 1) + column;
                        if self.is_hole(cell) {
                            continue;
                        }
                        let corners = self.cell_corners(column, row);
                        let footprint = cell_bounds(&corners);
                        let expanded = Aabb::new(footprint.min - extent, footprint.max + extent);
                        if expanded.ray_fraction(origin, translation, best).is_none() {
                            continue;
                        }
                        for half in 0..2 {
                            best = visit(self.cell_triangle(column, row, half, &corners), best)
                                .min(best);
                        }
                    }
                }
                best
            },
        )
    }

    fn overlap_candidates(&self, bounds: Aabb, visit: impl FnMut(Triangle) -> bool) -> bool {
        self.for_each_triangle(bounds, visit)
    }

    fn overlapping(&self, bounds: Aabb, mut visit: impl FnMut(u32) -> bool) -> bool {
        self.for_each_triangle(bounds, |triangle| visit(triangle.index))
    }
}
