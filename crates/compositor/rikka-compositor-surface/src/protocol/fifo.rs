use rikka_compositor_protocols::wp_fifo_manager_v1::{self, WpFifoManagerV1};
use rikka_compositor_protocols::wp_fifo_v1::{self, WpFifoV1};

use super::extension::{Claim, claim, target};
use crate::host::SurfaceHost;

pub(crate) fn manager<S: SurfaceHost>(
    state: &mut S,
    manager: WpFifoManagerV1,
    request: wp_fifo_manager_v1::Request,
) {
    let wp_fifo_manager_v1::Request::GetFifo { id, surface } = request else {
        return;
    };
    match claim(state, &surface, |extensions| {
        !std::mem::replace(&mut extensions.fifo, true)
    }) {
        Claim::Claimed(surface) => id.set_key(surface),
        Claim::Occupied => manager.post_error(wp_fifo_manager_v1::Error::AlreadyExists),
        Claim::Missing => {}
    }
}

pub(crate) fn request<S: SurfaceHost>(state: &mut S, fifo: WpFifoV1, request: wp_fifo_v1::Request) {
    if let wp_fifo_v1::Request::Destroy = request {
        return;
    }
    let Some(surface) = target(state, &fifo) else {
        fifo.post_error(wp_fifo_v1::Error::SurfaceDestroyed);
        return;
    };
    let constraints = &mut surface.pending.update.constraints;
    match request {
        wp_fifo_v1::Request::SetBarrier => constraints.set_barrier = true,
        wp_fifo_v1::Request::WaitBarrier => constraints.wait_barrier = true,
        wp_fifo_v1::Request::Destroy => {}
    }
}

pub(crate) fn destroyed<S: SurfaceHost>(state: &mut S, fifo: WpFifoV1) {
    if let Some(surface) = target(state, &fifo) {
        surface.extensions.fifo = false;
    }
}
