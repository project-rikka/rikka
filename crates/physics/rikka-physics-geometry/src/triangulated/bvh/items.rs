use rikka_physics_math::{Aabb, F32x4, Vector};

use super::bounds4::{Bounds4, QueryBounds4};
use crate::convex::extrema::{larger, smaller};

const PADDING: usize = 3;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ItemBounds {
    min: [Vec<f32>; 3],
    max: [Vec<f32>; 3],
}

#[inline(always)]
fn lanes(values: &[f32], position: usize) -> F32x4 {
    let window = &values[position..position + 4];
    F32x4::from_array([window[0], window[1], window[2], window[3]])
}

pub(crate) struct ItemSlices<'a> {
    min: [&'a mut [f32]; 3],
    max: [&'a mut [f32]; 3],
}

impl ItemSlices<'_> {
    pub(crate) fn fill(&mut self, items: &[u32], bounds: &[Aabb]) {
        let [min_x, min_y, min_z] = &mut self.min;
        let [max_x, max_y, max_z] = &mut self.max;
        for (offset, &item) in items.iter().enumerate() {
            let aabb = &bounds[item as usize];
            min_x[offset] = aabb.min.x;
            min_y[offset] = aabb.min.y;
            min_z[offset] = aabb.min.z;
            max_x[offset] = aabb.max.x;
            max_y[offset] = aabb.max.y;
            max_z[offset] = aabb.max.z;
        }
    }
}

impl ItemBounds {
    pub(crate) fn assign(&mut self, items: &[u32], bounds: &[Aabb]) {
        let len = items.len() + PADDING;
        for axis in 0..3 {
            self.min[axis].clear();
            self.min[axis].resize(len, f32::MAX);
            self.max[axis].clear();
            self.max[axis].resize(len, -f32::MAX);
        }
        self.update(items, 0, bounds);
    }

    pub(crate) fn update(&mut self, items: &[u32], start: usize, bounds: &[Aabb]) {
        self.slices(start, items.len()).fill(items, bounds);
    }

    fn slices(&mut self, start: usize, len: usize) -> ItemSlices<'_> {
        let range = start..start + len;
        let [min_x, min_y, min_z] = &mut self.min;
        let [max_x, max_y, max_z] = &mut self.max;
        ItemSlices {
            min: [
                &mut min_x[range.clone()],
                &mut min_y[range.clone()],
                &mut min_z[range.clone()],
            ],
            max: [
                &mut max_x[range.clone()],
                &mut max_y[range.clone()],
                &mut max_z[range],
            ],
        }
    }

    pub(crate) fn chunks(
        &mut self,
        len: usize,
        width: usize,
    ) -> impl Iterator<Item = ItemSlices<'_>> {
        let [min_x, min_y, min_z] = &mut self.min;
        let [max_x, max_y, max_z] = &mut self.max;
        min_x[..len]
            .chunks_mut(width)
            .zip(min_y[..len].chunks_mut(width))
            .zip(min_z[..len].chunks_mut(width))
            .zip(max_x[..len].chunks_mut(width))
            .zip(max_y[..len].chunks_mut(width))
            .zip(max_z[..len].chunks_mut(width))
            .map(
                |(((((min_x, min_y), min_z), max_x), max_y), max_z)| ItemSlices {
                    min: [min_x, min_y, min_z],
                    max: [max_x, max_y, max_z],
                },
            )
    }

    pub(crate) fn union(&self, start: u32, count: u32) -> Aabb {
        let range = start as usize..(start + count) as usize;
        let low = |values: &[f32]| {
            values[range.clone()]
                .iter()
                .fold(f32::MAX, |a, &b| smaller(a, b))
        };
        let high = |values: &[f32]| {
            values[range.clone()]
                .iter()
                .fold(-f32::MAX, |a, &b| larger(a, b))
        };
        Aabb::new(
            [low(&self.min[0]), low(&self.min[1]), low(&self.min[2])].into(),
            [high(&self.max[0]), high(&self.max[1]), high(&self.max[2])].into(),
        )
    }

    #[inline(always)]
    pub(crate) fn lanes(&self, position: usize) -> Bounds4 {
        Bounds4 {
            min: [
                lanes(&self.min[0], position),
                lanes(&self.min[1], position),
                lanes(&self.min[2], position),
            ],
            max: [
                lanes(&self.max[0], position),
                lanes(&self.max[1], position),
                lanes(&self.max[2], position),
            ],
        }
    }

    pub(crate) fn overlapping(
        &self,
        start: u32,
        count: u32,
        query: &QueryBounds4,
    ) -> impl Iterator<Item = u32> {
        let end = start + count;
        let query = *query;
        (start..end).step_by(4).flat_map(move |chunk| {
            let mask = self.lanes(chunk as usize).overlaps_query(&query) & lane_mask(end - chunk);
            (0..4u32)
                .filter(move |lane| mask & (1 << lane) != 0)
                .map(move |lane| chunk + lane)
        })
    }
}

#[inline]
pub(crate) fn lane_mask(remaining: u32) -> u32 {
    (1u32 << remaining.min(4)) - 1
}
