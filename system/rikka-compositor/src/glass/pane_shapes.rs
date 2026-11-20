use std::sync::atomic::{AtomicU64, Ordering};

use rikka_compositor_geometry::{Logical, Rect, Region, Size};
use rikka_compositor_panes::read_panes;
use rikka_types::RoundedRect;

static NEXT_KEY: AtomicU64 = AtomicU64::new(1);

pub(crate) fn next_key() -> u64 {
    NEXT_KEY.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, Default)]
pub(super) struct PaneShapes {
    region: Vec<Rect<i32, Logical>>,
    over: Size<i32, Logical>,
    shapes: Vec<RoundedRect>,
    keys: Vec<u64>,
}

impl PaneShapes {
    pub(super) fn panes(
        &mut self,
        region: &Region<Logical>,
        size: Size<i32, Logical>,
    ) -> impl Iterator<Item = (u64, RoundedRect)> + '_ {
        if self.over != size || !region.iter().eq(self.region.iter().copied()) {
            self.region = region.iter().collect();
            self.over = size;
            self.shapes.clear();
            read_panes(region, size, |shape| self.shapes.push(shape));
            self.keys.resize_with(self.shapes.len(), next_key);
        }
        self.held()
    }

    pub(super) fn held(&self) -> impl Iterator<Item = (u64, RoundedRect)> + '_ {
        std::iter::zip(self.keys.iter().copied(), self.shapes.iter().copied())
    }
}
