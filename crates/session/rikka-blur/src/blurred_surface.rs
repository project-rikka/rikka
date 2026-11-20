use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rikka_types::RoundedRect;
use rikka_wayland_client::ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1;
use rikka_wayland_client::wl_compositor::WlCompositor;

use crate::shape;

#[must_use]
pub struct BlurredSurface {
    effect: ExtBackgroundEffectSurfaceV1,
    compositor: WlCompositor,
    blurs: Arc<AtomicBool>,
    regions: RefCell<Vec<RoundedRect>>,
    sent: Cell<bool>,
}

impl BlurredSurface {
    pub(crate) fn new(
        effect: ExtBackgroundEffectSurfaceV1,
        compositor: WlCompositor,
        blurs: Arc<AtomicBool>,
    ) -> BlurredSurface {
        BlurredSurface {
            effect,
            compositor,
            blurs,
            regions: RefCell::new(Vec::new()),
            sent: Cell::new(false),
        }
    }

    pub fn set_region(&self, panes: &[RoundedRect]) {
        let mut regions = self.regions.borrow_mut();
        if *regions == panes && self.sent.get() {
            return;
        }
        regions.clear();
        regions.extend_from_slice(panes);
        drop(regions);
        let blurs = self.blurs.load(Ordering::Acquire);
        self.sent.set(blurs);
        if !blurs {
            return;
        }

        if panes.is_empty() {
            self.effect.set_blur_region(None);
            return;
        }
        let mut rectangles = Vec::new();
        for pane in panes {
            shape::rectangles(*pane, &mut rectangles);
        }
        let region = self.compositor.create_region();
        for [x, y, width, height] in rectangles {
            region.add(x, y, width, height);
        }
        self.effect.set_blur_region(Some(&region));
        region.destroy();
    }
}

impl Drop for BlurredSurface {
    fn drop(&mut self) {
        self.effect.destroy();
    }
}

impl std::fmt::Debug for BlurredSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlurredSurface")
            .field("panes", &self.regions.borrow().len())
            .finish_non_exhaustive()
    }
}
