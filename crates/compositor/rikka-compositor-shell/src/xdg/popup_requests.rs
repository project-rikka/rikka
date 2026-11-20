use rikka_compositor_protocols::xdg_popup::{Error, Request, XdgPopup};
use rikka_compositor_protocols::xdg_wm_base;

use super::popup::PopupParent;
use super::popup_lifecycle::{self, emit, popup_of};
use super::positioner_requests::Draft;
use crate::event::PopupEvent;
use crate::host::ShellHost;
use crate::ids::PopupId;

pub(crate) fn request<S: ShellHost>(state: &mut S, popup: XdgPopup, request: Request) {
    let Some(id) = popup_of(state, &popup) else {
        return;
    };
    let shell = state.shell();
    let Some(record) = shell.popups.get(id) else {
        return;
    };
    let wm_base = record.wm_base.clone();
    match request {
        Request::Destroy => {
            let covered = shell.popups.values().any(|other| {
                other.parent == Some(PopupParent::Popup(id)) && other.resource.alive()
            });
            if covered {
                wm_base.post_error(xdg_wm_base::Error::NotTheTopmostPopup);
            }
        }
        Request::Grab { seat, serial } => {
            if record.lifecycle.initialized() {
                return popup.post_error(Error::InvalidGrab);
            }
            emit(state, id, PopupEvent::Grab { seat, serial });
        }
        Request::Reposition { positioner, token } => {
            let Some(positioner) = Draft::lookup(state.shell(), &positioner) else {
                return wm_base.post_error(xdg_wm_base::Error::InvalidPositioner);
            };
            if let Some(record) = state.shell().popups.get_mut(id) {
                record.positioner = positioner;
                record.reposition = Some(token);
            }
            emit(state, id, PopupEvent::Reposition { token });
        }
    }
}

pub(crate) fn destroyed<S: ShellHost>(state: &mut S, popup: XdgPopup) {
    if let Some(id) = popup.key::<PopupId>() {
        popup_lifecycle::remove(state, id);
    }
}
