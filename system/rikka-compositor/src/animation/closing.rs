use std::time::Instant;

use rikka_compositor_desktop::{SnapshotId, WindowId};
use rikka_compositor_geometry::{Logical, Rect};

use super::motion::WindowMotion;
use crate::render::Visual;

#[derive(Debug)]
pub(crate) struct Closing {
    pub(crate) window: Option<WindowId>,
    pub(crate) snapshot: SnapshotId,
    pub(crate) geometry: Rect<i32, Logical>,
    motion: WindowMotion,
}

impl Closing {
    pub(crate) fn new(
        window: Option<WindowId>,
        snapshot: SnapshotId,
        geometry: Rect<i32, Logical>,
        now: Instant,
    ) -> Self {
        let mut motion = WindowMotion::at_rest();
        motion.close(now);
        Self {
            window,
            snapshot,
            geometry,
            motion,
        }
    }

    pub(crate) fn advance(&mut self, now: Instant) {
        self.motion.advance(now);
    }

    pub(crate) fn is_finished(&self) -> bool {
        !self.motion.is_animating()
    }

    pub(crate) fn shown(&mut self, now: Instant) -> Visual {
        self.motion
            .sample(now, self.geometry)
            .unwrap_or_else(|| Visual::opaque(self.geometry))
    }
}
