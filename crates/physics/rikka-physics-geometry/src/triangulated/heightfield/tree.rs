use std::ops::Range;

use rikka_physics_math::{Aabb, F32x4, Vec3, Vector};

use super::data::{LEVEL_SHIFT, LOCAL_MASK, Level};
use super::view::HeightFieldView;
use crate::triangulated::bvh::{Bounds4, Child, Nodes};

#[derive(Clone, Copy, Debug)]
pub(crate) struct HeightFieldTree<'a> {
    pub(crate) field: &'a HeightFieldView<'a>,
}

impl HeightFieldTree<'_> {
    #[inline]
    fn level(&self, node: u32) -> (u32, Level, u32, u32) {
        let index = node >> LEVEL_SHIFT;
        let level = self.field.levels[index as usize];
        let local = node & LOCAL_MASK;
        (index, level, local % level.width, local / level.width)
    }

    #[inline]
    fn child_at(&self, node: u32, slot: usize) -> Option<(u32, Level, u32, u32)> {
        let (index, _, x, z) = self.level(node);
        let child_level = index.checked_sub(1)?;
        let level = self.field.levels[child_level as usize];
        let cx = 2 * x + (slot as u32 & 1);
        let cz = 2 * z + (slot as u32 >> 1);
        (cx < level.width && cz < level.depth).then_some((child_level, level, cx, cz))
    }

    #[inline]
    fn range(&self, level: &Level, x: u32, z: u32) -> [u16; 2] {
        self.field.ranges[(level.offset + z * level.width + x) as usize]
    }

    pub(crate) fn block_cells(&self, block: u32) -> (Range<u32>, Range<u32>) {
        let level = self.field.levels[0];
        let (x, z) = (block % level.width, block / level.width);
        cell_span(
            &level,
            x,
            z,
            self.field.columns() - 1,
            self.field.rows() - 1,
        )
    }
}

#[inline]
fn cell_span(
    level: &Level,
    x: u32,
    z: u32,
    cells_x: u32,
    cells_z: u32,
) -> (Range<u32>, Range<u32>) {
    let x0 = x << level.shift;
    let z0 = z << level.shift;
    (
        x0.min(cells_x)..((x + 1) << level.shift).min(cells_x),
        z0.min(cells_z)..((z + 1) << level.shift).min(cells_z),
    )
}

impl Nodes for HeightFieldTree<'_> {
    type Frame = ();

    #[inline]
    fn root(&self) -> Option<(u32, ())> {
        Some((self.field.header.root, ()))
    }

    fn load(&self, node: u32, _: &()) -> Bounds4 {
        let scale = self.field.scale();
        let cells_x = self.field.columns() - 1;
        let cells_z = self.field.rows() - 1;
        let mut lanes = [Aabb::EMPTY; 4];
        for (slot, lane) in lanes.iter_mut().enumerate() {
            let Some((_, level, x, z)) = self.child_at(node, slot) else {
                continue;
            };
            let [low, high] = self.range(&level, x, z);
            if low > high {
                continue;
            }
            let (columns, rows) = cell_span(&level, x, z, cells_x, cells_z);
            *lane = Aabb::new(
                Vec3::new(
                    scale.x * columns.start as f32,
                    self.field.decode_height(low),
                    scale.z * rows.start as f32,
                ),
                Vec3::new(
                    scale.x * columns.end as f32,
                    self.field.decode_height(high),
                    scale.z * rows.end as f32,
                ),
            );
        }
        let pack = |f: fn(&Aabb) -> f32| F32x4::from_array(lanes.each_ref().map(f));
        Bounds4 {
            min: [pack(|b| b.min.x), pack(|b| b.min.y), pack(|b| b.min.z)],
            max: [pack(|b| b.max.x), pack(|b| b.max.y), pack(|b| b.max.z)],
        }
    }

    #[inline]
    fn child(&self, node: u32, slot: usize) -> Child {
        match self.child_at(node, slot) {
            None => Child::Empty,
            Some((0, level, x, z)) => Child::Leaf {
                start: z * level.width + x,
                count: 1,
            },
            Some((index, level, x, z)) => Child::Node(index << LEVEL_SHIFT | (z * level.width + x)),
        }
    }

    #[inline]
    fn occupied(&self, node: u32) -> u32 {
        (0..4).fold(0, |mask, slot| {
            let solid = self.child_at(node, slot).is_some_and(|(_, level, x, z)| {
                let [low, high] = self.range(&level, x, z);
                low <= high
            });
            mask | (u32::from(solid) << slot)
        })
    }

    #[inline]
    fn child_frame(_: &Bounds4, _: usize) {}
}
