use rikka_compositor_protocols::wp_fractional_scale_manager_v1::{
    self, WpFractionalScaleManagerV1,
};
use rikka_compositor_protocols::wp_fractional_scale_v1::{self, WpFractionalScaleV1};

use super::extension::{Claim, claim, target};
use crate::host::SurfaceHost;
use crate::output::send_fractional;

pub(crate) fn manager<S: SurfaceHost>(
    state: &mut S,
    manager: WpFractionalScaleManagerV1,
    request: wp_fractional_scale_manager_v1::Request,
) {
    let wp_fractional_scale_manager_v1::Request::GetFractionalScale { id, surface } = request
    else {
        return;
    };
    let claimed = claim(state, &surface, |extensions| {
        extensions.fractional.is_none() && {
            extensions.fractional = Some(id.clone());
            true
        }
    });
    match claimed {
        Claim::Claimed(surface) => {
            id.set_key(surface);
            if let Some(surface) = state.surfaces().get(surface) {
                send_fractional(surface);
            }
        }
        Claim::Occupied => {
            manager.post_error(wp_fractional_scale_manager_v1::Error::FractionalScaleExists)
        }
        Claim::Missing => {}
    }
}

pub(crate) fn request<S>(
    _state: &mut S,
    _fractional: WpFractionalScaleV1,
    _request: wp_fractional_scale_v1::Request,
) {
}

pub(crate) fn destroyed<S: SurfaceHost>(state: &mut S, fractional: WpFractionalScaleV1) {
    if let Some(surface) = target(state, &fractional)
        && surface.extensions.fractional.as_ref() == Some(&fractional)
    {
        surface.extensions.fractional = None;
    }
}
