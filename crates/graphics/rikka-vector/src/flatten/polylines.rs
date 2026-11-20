use std::ops::Range;

use rikka_simd::{Float, Mask};
use rikka_types::Point;

use crate::flatten::flatness::Flatness;
use crate::flatten::flattener::{Columns, Flattening, flatten_points};
use crate::lanes::{LANES, slots};
use crate::path::Path;
use crate::transform::Transform;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub x_min: f32,
    pub y_min: f32,
    pub x_max: f32,
    pub y_max: f32,
    pub finite: bool,
}

impl Bounds {
    #[inline]
    #[must_use]
    pub fn is_solid(&self) -> bool {
        self.x_min < self.x_max && self.y_min < self.y_max
    }
}

#[derive(Debug, Default)]
pub struct Polylines {
    pub(crate) xs: Vec<f32>,
    pub(crate) ys: Vec<f32>,
    pub(crate) ends: Vec<usize>,
    device: Vec<Point>,
    steps: Vec<u16>,
}

impl Polylines {
    pub fn flatten(
        &mut self,
        path: &Path,
        transform: &Transform,
        tolerance: f32,
        limit: usize,
    ) -> bool {
        self.device.clear();
        self.device
            .extend(path.points().iter().map(|&point| transform.apply(point)));
        flatten_points(
            path.verbs(),
            &self.device,
            Flattening {
                flatness: Flatness::new(tolerance),
                limit,
                steps: &mut self.steps,
            },
            Columns {
                xs: &mut self.xs,
                ys: &mut self.ys,
                ends: &mut self.ends,
            },
            None,
        )
    }

    #[inline(always)]
    pub fn columns(&mut self, len: usize) -> (&mut [f32], &mut [f32], &mut Vec<usize>) {
        (
            slots(&mut self.xs, len),
            slots(&mut self.ys, len),
            &mut self.ends,
        )
    }

    #[inline(always)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.ends.last().copied().unwrap_or(0)
    }

    #[inline(always)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn contours(&self) -> impl Iterator<Item = Range<usize>> + '_ {
        let mut start = 0;
        self.ends.iter().map(move |&end| {
            let contour = start..end;
            start = end;
            contour
        })
    }

    #[inline(always)]
    pub fn bounds<V: Float<Elem = f32>>(&self) -> Option<Bounds> {
        let len = self.len();
        if len == 0 {
            return None;
        }
        let (x_min, x_max, x_finite) = extent::<V>(&self.xs, len);
        let (y_min, y_max, y_finite) = extent::<V>(&self.ys, len);
        Some(Bounds {
            x_min,
            y_min,
            x_max,
            y_max,
            finite: x_finite && y_finite,
        })
    }
}

#[inline(always)]
fn extent<V: Float<Elem = f32>>(values: &[f32], len: usize) -> (f32, f32, bool) {
    let first = V::splat(values[0]);
    let (mut low, mut high) = (first, first);
    let mut finite = V::Mask::splat(true);
    for at in (0..len.next_multiple_of(LANES)).step_by(V::LANES) {
        let value = V::load(&values[at..]);
        low = value.min(low);
        high = value.max(high);
        finite &= value.is_finite();
    }
    (low.reduce_min(), high.reduce_max(), finite.all())
}
