use rikka_wayland::Host;
use rikka_wayland_client::wl_compositor::WlCompositor;
use rikka_wayland_client::wl_surface::WlSurface;

use crate::opaque_region::OpaqueRegion;

#[derive(Debug, Clone)]
pub struct OpaqueRegions {
    compositor: WlCompositor,
}

impl OpaqueRegions {
    pub fn bind(host: &Host) -> Option<OpaqueRegions> {
        let compositor = host
            .display()
            .bind::<WlCompositor>(1..=6)
            .inspect_err(
                |error| tracing::debug!(%error, "wl_compositor unavailable for opaque regions"),
            )
            .ok()?;
        Some(OpaqueRegions { compositor })
    }

    pub fn for_surface(&self, surface: WlSurface) -> OpaqueRegion {
        OpaqueRegion::new(surface, self.compositor.clone())
    }
}
