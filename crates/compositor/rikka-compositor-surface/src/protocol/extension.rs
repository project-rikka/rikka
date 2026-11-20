use rikka_compositor_protocols::wl_surface::WlSurface;
use rikka_wayland_wire::Resource;

use crate::host::SurfaceHost;
use crate::ids::SurfaceId;
use crate::surface::{Extensions, Surface};

pub(super) enum Claim {
    Claimed(SurfaceId),
    Occupied,
    Missing,
}

pub(super) fn claim<S: SurfaceHost>(
    state: &mut S,
    surface: &WlSurface,
    slot: impl FnOnce(&mut Extensions) -> bool,
) -> Claim {
    let surfaces = state.surfaces();
    let Some(id) = surfaces.lookup(surface) else {
        return Claim::Missing;
    };
    let Some(target) = surfaces.surfaces.get_mut(id) else {
        return Claim::Missing;
    };
    if slot(&mut target.extensions) {
        Claim::Claimed(id)
    } else {
        Claim::Occupied
    }
}

pub(super) fn target<'a, S: SurfaceHost>(
    state: &'a mut S,
    resource: &impl Resource,
) -> Option<&'a mut Surface> {
    let id = resource.object().key::<SurfaceId>()?;
    state.surfaces().surfaces.get_mut(id)
}
