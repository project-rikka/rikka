use rikka_compositor_protocols::xdg_popup::XdgPopup;
use rikka_compositor_protocols::xdg_positioner::XdgPositioner;
use rikka_compositor_protocols::xdg_surface::{self, XdgSurface};
use rikka_compositor_protocols::xdg_wm_base::{self, XdgWmBase};
use rikka_compositor_surface::SurfaceId;

use super::popup::{Popup, PopupParent};
use super::positioner_requests::Draft;
use super::window::WindowState;
use super::xdg_role::{XdgKind, XdgObject, XdgRole};
use crate::event::{PopupEvent, ShellEvent};
use crate::host::ShellHost;
use crate::ids::PopupId;
use crate::lifecycle::{Lifecycle, Transition};

#[derive(Debug)]
pub(crate) struct CreateParams {
    pub(crate) xdg_surface: XdgSurface,
    pub(crate) wm_base: XdgWmBase,
    pub(crate) resource: XdgPopup,
    pub(crate) parent: Option<XdgSurface>,
    pub(crate) positioner: XdgPositioner,
}

pub(crate) fn popup_of<S: ShellHost>(state: &mut S, popup: &XdgPopup) -> Option<PopupId> {
    popup
        .key::<PopupId>()
        .filter(|id| state.shell().popups.contains_key(*id))
}

pub(crate) fn create<S: ShellHost>(state: &mut S, surface: SurfaceId, params: CreateParams) {
    let CreateParams {
        xdg_surface,
        wm_base,
        resource,
        parent,
        positioner,
    } = params;
    let Some(positioner) = Draft::lookup(state.shell(), &positioner) else {
        return wm_base.post_error(xdg_wm_base::Error::InvalidPositioner);
    };
    let parent = match parent {
        None => None,
        Some(parent) => match parent_of(state, &parent) {
            Some(parent) => Some(parent),
            None => return wm_base.post_error(xdg_wm_base::Error::InvalidPopupParent),
        },
    };
    let Some(role) = state.surfaces().role::<XdgRole>(surface) else {
        return;
    };
    if role.object.is_some() {
        return xdg_surface.post_error(xdg_surface::Error::AlreadyConstructed);
    }
    let Some(wl_surface) = state
        .surfaces()
        .get(surface)
        .map(|surface| surface.resource().clone())
    else {
        return;
    };
    if wl_surface.key::<XdgKind>() == Some(XdgKind::Toplevel) {
        return wm_base.post_error(xdg_wm_base::Error::Role);
    }
    wl_surface.set_key(XdgKind::Popup);
    let id = state.shell().popups.insert(Popup {
        resource: resource.clone(),
        xdg_surface,
        wm_base,
        surface,
        lifecycle: Lifecycle::new(WindowState::default()),
        parent,
        positioner,
        reposition: None,
        dismissed: false,
    });
    resource.set_key(id);
    if let Some(role) = state.surfaces().role_mut::<XdgRole>(surface) {
        role.object = Some(XdgObject::Popup(id));
        role.constructed = true;
    }
    emit(state, id, PopupEvent::Created);
}

pub(crate) fn remove<S: ShellHost>(state: &mut S, id: PopupId) {
    let Some(record) = state.shell().popups.remove(id) else {
        return;
    };
    if let Some(role) = state.surfaces().role_mut::<XdgRole>(record.surface)
        && role.object == Some(XdgObject::Popup(id))
    {
        role.object = None;
    }
    state.shell().dismiss_children(PopupParent::Popup(id));
    emit(state, id, PopupEvent::Destroyed);
}

pub(crate) fn committed<S: ShellHost>(state: &mut S, id: PopupId, has_buffer: bool) {
    let Some(record) = state.shell().popups.get_mut(id) else {
        return;
    };
    let applied = record.lifecycle.apply(has_buffer, &WindowState::default());
    if applied.transition == Some(Transition::Unmapped) {
        record.reposition = None;
    }
    if let Some(serial) = applied.configured {
        emit(state, id, PopupEvent::Configured { serial });
    }
    let event = match applied.transition {
        Some(Transition::InitialCommit) => PopupEvent::InitialCommit,
        Some(Transition::Mapped) => PopupEvent::Mapped,
        Some(Transition::Unmapped) => PopupEvent::Unmapped,
        None => return,
    };
    emit(state, id, event);
}

pub(crate) fn emit<S: ShellHost>(state: &mut S, popup: PopupId, event: PopupEvent) {
    state.shell_event(ShellEvent::Popup { popup, event });
}

fn parent_of<S: ShellHost>(state: &mut S, parent: &XdgSurface) -> Option<PopupParent> {
    let surface = parent.key::<SurfaceId>()?;
    let role = state.surfaces().role::<XdgRole>(surface)?;
    if role.xdg_surface != *parent {
        return None;
    }
    Some(match role.object? {
        XdgObject::Toplevel(toplevel) => PopupParent::Toplevel(toplevel),
        XdgObject::Popup(popup) => PopupParent::Popup(popup),
    })
}
