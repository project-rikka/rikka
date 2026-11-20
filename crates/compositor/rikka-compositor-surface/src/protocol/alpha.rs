use rikka_compositor_protocols::wp_alpha_modifier_surface_v1::{self, WpAlphaModifierSurfaceV1};
use rikka_compositor_protocols::wp_alpha_modifier_v1::{self, WpAlphaModifierV1};

use super::extension::{Claim, claim, target};
use crate::host::SurfaceHost;

pub(crate) fn manager<S: SurfaceHost>(
    state: &mut S,
    manager: WpAlphaModifierV1,
    request: wp_alpha_modifier_v1::Request,
) {
    let wp_alpha_modifier_v1::Request::GetSurface { id, surface } = request else {
        return;
    };
    match claim(state, &surface, |extensions| {
        !std::mem::replace(&mut extensions.alpha, true)
    }) {
        Claim::Claimed(surface) => id.set_key(surface),
        Claim::Occupied => manager.post_error(wp_alpha_modifier_v1::Error::AlreadyConstructed),
        Claim::Missing => {}
    }
}

pub(crate) fn request<S: SurfaceHost>(
    state: &mut S,
    object: WpAlphaModifierSurfaceV1,
    request: wp_alpha_modifier_surface_v1::Request,
) {
    let wp_alpha_modifier_surface_v1::Request::SetMultiplier { factor } = request else {
        return;
    };
    match target(state, &object) {
        Some(surface) => surface.pending.update.alpha = Some(factor),
        None => object.post_error(wp_alpha_modifier_surface_v1::Error::NoSurface),
    }
}

pub(crate) fn destroyed<S: SurfaceHost>(state: &mut S, object: WpAlphaModifierSurfaceV1) {
    if let Some(surface) = target(state, &object) {
        surface.extensions.alpha = false;
        surface.pending.update.alpha = Some(u32::MAX);
    }
}
