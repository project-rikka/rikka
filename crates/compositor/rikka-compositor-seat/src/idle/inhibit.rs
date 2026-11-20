use rikka_compositor_protocols::zwp_idle_inhibit_manager_v1::{
    self as manager, ZwpIdleInhibitManagerV1,
};
use rikka_compositor_protocols::zwp_idle_inhibitor_v1::{self as protocol, ZwpIdleInhibitorV1};
use rikka_compositor_surface::SurfaceId;

use crate::access::Access;
use crate::event::SeatEvent;
use crate::idle::Inhibitor;
use crate::surface;

pub(crate) fn idle_inhibit_manager<A: Access>(
    state: &mut A,
    _manager: ZwpIdleInhibitManagerV1,
    request: manager::Request,
) {
    let manager::Request::CreateInhibitor { id, surface } = request else {
        return;
    };
    let Some(surface) = surface::id(state, &surface) else {
        return;
    };
    state.seats().idle.inhibitors.push(Inhibitor {
        resource: id,
        surface,
    });
    state.report(SeatEvent::IdleInhibitors);
}

pub(crate) fn idle_inhibitor<A: Access>(
    _state: &mut A,
    _inhibitor: ZwpIdleInhibitorV1,
    _request: protocol::Request,
) {
}

pub(crate) fn idle_inhibitor_destroyed<A: Access>(state: &mut A, inhibitor: ZwpIdleInhibitorV1) {
    let inhibitors = &mut state.seats().idle.inhibitors;
    let before = inhibitors.len();
    inhibitors.retain(|inhibition| inhibition.resource != inhibitor);
    if inhibitors.len() != before {
        state.report(SeatEvent::IdleInhibitors);
    }
}

pub(crate) fn inhibitors_surface_gone<A: Access>(state: &mut A, surface: SurfaceId) {
    let inhibitors = &mut state.seats().idle.inhibitors;
    let before = inhibitors.len();
    inhibitors.retain(|inhibition| inhibition.surface != surface);
    if inhibitors.len() != before {
        state.report(SeatEvent::IdleInhibitors);
    }
}
