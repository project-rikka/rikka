use rikka_compositor_protocols::wl_surface::WlSurface;
use rikka_compositor_surface::SurfaceId;

use crate::access::Access;

pub(crate) fn resource<A: Access>(state: &mut A, surface: SurfaceId) -> Option<WlSurface> {
    state
        .surfaces()
        .get(surface)
        .map(|surface| surface.resource().clone())
        .filter(WlSurface::alive)
}

pub(crate) fn id<A: Access>(state: &mut A, surface: &WlSurface) -> Option<SurfaceId> {
    state.surfaces().lookup(surface)
}
