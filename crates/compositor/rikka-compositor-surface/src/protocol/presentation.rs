use rikka_compositor_protocols::wp_presentation::{Request, WpPresentation};

use crate::host::SurfaceHost;

const MONOTONIC_CLOCK: u32 = libc::CLOCK_MONOTONIC as u32;

pub(crate) fn bound<S>(_state: &mut S, presentation: WpPresentation) {
    presentation.clock_id(MONOTONIC_CLOCK);
}

pub(crate) fn request<S: SurfaceHost>(
    state: &mut S,
    _presentation: WpPresentation,
    request: Request,
) {
    let Request::Feedback { surface, callback } = request else {
        return;
    };
    let surfaces = state.surfaces();
    match surfaces
        .lookup(&surface)
        .and_then(|surface| surfaces.surfaces.get_mut(surface))
    {
        Some(target) => target.pending.update.feedback.push(callback),
        None => callback.discarded(),
    }
}
