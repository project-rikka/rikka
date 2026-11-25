mod dividing;
mod moving;
mod popup;
mod press;
mod request;
mod requests;
mod resizing;
mod retiling;

use rikka_compositor_seat::{Button, PointerHandle};

pub(super) use moving::Drag;
pub(crate) use popup::PopupChains;
pub(crate) use request::{GrabRequest, GrabRequests};
pub(super) use resizing::Reshape;

use crate::state::Compositor;

#[derive(Debug, Default)]
pub(crate) struct Grabs {
    pub(crate) popups: PopupChains,
    pub(crate) requests: GrabRequests,
}

pub(super) fn release(pointer: &mut PointerHandle<'_, Compositor>, button: &Button) {
    pointer.button(button);
    if !pointer.pressed() {
        pointer.end_grab();
    }
}
