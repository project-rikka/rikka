use std::cell::RefCell;

use rikka_types::RoundedRect;
use rikka_wayland_client::wl_compositor::WlCompositor;
use rikka_wayland_client::wl_surface::WlSurface;

use crate::opaque_rectangles::{Area, opaque_rectangles};

#[must_use]
pub struct OpaqueRegion {
    surface: WlSurface,
    compositor: WlCompositor,
    regions: RefCell<Option<Vec<Area>>>,
}

impl OpaqueRegion {
    pub(crate) fn new(surface: WlSurface, compositor: WlCompositor) -> OpaqueRegion {
        OpaqueRegion {
            surface,
            compositor,
            regions: RefCell::new(None),
        }
    }

    pub fn set_region(&self, opaque: &[RoundedRect], glass: &[RoundedRect]) {
        let mut areas = Vec::new();
        opaque_rectangles(opaque, glass, &mut areas);
        let mut regions = self.regions.borrow_mut();
        if regions.as_ref() == Some(&areas) {
            return;
        }
        if areas.is_empty() {
            self.surface.set_opaque_region(None);
        } else {
            let region = self.compositor.create_region();
            for [x, y, width, height] in &areas {
                region.add(*x, *y, *width, *height);
            }
            self.surface.set_opaque_region(Some(&region));
            region.destroy();
        }
        *regions = Some(areas);
    }
}

impl std::fmt::Debug for OpaqueRegion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpaqueRegion")
            .field("areas", &self.regions.borrow().as_ref().map(Vec::len))
            .finish_non_exhaustive()
    }
}
