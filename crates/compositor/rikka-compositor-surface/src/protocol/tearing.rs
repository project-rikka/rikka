use rikka_compositor_protocols::wp_tearing_control_manager_v1::{self, WpTearingControlManagerV1};
use rikka_compositor_protocols::wp_tearing_control_v1::{self, WpTearingControlV1};
use rikka_compositor_server::Client;
use rikka_compositor_types::TearingHint;

use super::extension::{Claim, claim, target};
use crate::hints::tearing_hint;
use crate::host::SurfaceHost;

pub(crate) fn manager<S: SurfaceHost>(
    state: &mut S,
    manager: WpTearingControlManagerV1,
    request: wp_tearing_control_manager_v1::Request,
) {
    let wp_tearing_control_manager_v1::Request::GetTearingControl { id, surface } = request else {
        return;
    };
    match claim(state, &surface, |extensions| {
        !std::mem::replace(&mut extensions.tearing, true)
    }) {
        Claim::Claimed(surface) => id.set_key(surface),
        Claim::Occupied => {
            manager.post_error(wp_tearing_control_manager_v1::Error::TearingControlExists)
        }
        Claim::Missing => {}
    }
}

pub(crate) fn request<S: SurfaceHost>(
    state: &mut S,
    object: WpTearingControlV1,
    request: wp_tearing_control_v1::Request,
) {
    let wp_tearing_control_v1::Request::SetPresentationHint { hint } = request else {
        return;
    };
    let Ok(hint) = hint else {
        if let Some(client) = Client::from_resource(&object) {
            client.kill("unknown wp_tearing_control_v1 presentation hint");
        }
        return;
    };
    if let Some(surface) = target(state, &object) {
        surface.pending.update.tearing = Some(tearing_hint(hint));
    }
}

pub(crate) fn destroyed<S: SurfaceHost>(state: &mut S, object: WpTearingControlV1) {
    if let Some(surface) = target(state, &object) {
        surface.extensions.tearing = false;
        surface.pending.update.tearing = Some(TearingHint::Vsync);
    }
}
