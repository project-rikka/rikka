use rikka_compositor_geometry::{Logical, Rect};

use crate::column::MIN_EXTENT;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spacing {
    pub inner: i32,
    pub outer: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub work: Rect<i32, Logical>,
    pub full: Rect<i32, Logical>,
    pub spacing: Spacing,
}

impl Frame {
    pub(crate) fn span(&self) -> f64 {
        f64::from(self.work.size.w)
    }

    pub(crate) fn outer(&self) -> f64 {
        f64::from(self.spacing.outer)
    }

    pub(crate) fn proportion(&self, share: f64) -> i32 {
        let Spacing { inner, outer } = self.spacing;
        let room = f64::from(self.work.size.w - 2 * outer + inner);
        ((room * share - f64::from(inner)).round() as i32).max(MIN_EXTENT)
    }

    pub(crate) fn column_room(&self, cells: usize) -> f64 {
        let Spacing { inner, outer } = self.spacing;
        let gaps = 2 * outer + inner * (cells.max(1) as i32 - 1);
        f64::from((self.work.size.h - gaps).max(MIN_EXTENT * cells.max(1) as i32))
    }

    pub(crate) fn full_offset(&self) -> f64 {
        f64::from(self.work.loc.x - self.full.loc.x)
    }
}
