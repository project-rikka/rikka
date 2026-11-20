use rikka_compositor_geometry::{Point, Rect, Size};
use rikka_compositor_protocols::wl_surface::WlSurface;
use rikka_compositor_protocols::xdg_surface::{Error, Request, XdgSurface};
use rikka_compositor_protocols::xdg_wm_base::{self, XdgWmBase};
use rikka_compositor_surface::{Surface, SurfaceId, assign_role};

use super::xdg_role::{XdgObject, XdgRole};
use super::{popup_lifecycle, toplevel_lifecycle};
use crate::host::ShellHost;
use crate::ids::WmBaseId;

pub(crate) fn create<S: ShellHost>(
    state: &mut S,
    wm_base: &XdgWmBase,
    xdg_surface: XdgSurface,
    surface: &WlSurface,
) {
    let Some(id) = state.surfaces().lookup(surface) else {
        return;
    };
    xdg_surface.set_key(id);
    if state
        .surfaces()
        .get(id)
        .is_some_and(Surface::buffer_attached)
    {
        return xdg_surface.post_error(Error::UnconfiguredBuffer);
    }
    let role = XdgRole {
        xdg_surface: xdg_surface.clone(),
        wm_base: wm_base.clone(),
        object: None,
        constructed: false,
    };
    if let Err(error) = assign_role(state, id, role) {
        return error.post(wm_base, xdg_wm_base::Error::Role.code());
    }
    let Some(owner) = wm_base.key::<WmBaseId>() else {
        return;
    };
    xdg_surface.set_key(owner);
    if let Some(record) = state.shell().wm_bases.get_mut(owner) {
        record.surfaces += 1;
    }
}

pub(crate) fn request<S: ShellHost>(state: &mut S, xdg_surface: XdgSurface, request: Request) {
    let Some(surface) = xdg_surface.key::<SurfaceId>() else {
        return;
    };
    let Some(role) = state.surfaces().role::<XdgRole>(surface) else {
        return;
    };
    let object = role.object;
    let wm_base = role.wm_base.clone();
    match request {
        Request::Destroy => {
            if object.is_some() {
                xdg_surface.post_error(Error::DefunctRoleObject);
            }
        }
        Request::GetToplevel { id } => {
            toplevel_lifecycle::create(state, surface, xdg_surface, wm_base, id);
        }
        Request::GetPopup {
            id,
            parent,
            positioner,
        } => popup_lifecycle::create(
            state,
            surface,
            popup_lifecycle::CreateParams {
                xdg_surface,
                wm_base,
                resource: id,
                parent,
                positioner,
            },
        ),
        Request::SetWindowGeometry {
            x,
            y,
            width,
            height,
        } => {
            if width <= 0 || height <= 0 {
                return xdg_surface.post_error(Error::InvalidSize);
            }
            let geometry = Rect::new(Point::new(x, y), Size::new(width, height));
            edit_window(state, object, &xdg_surface, |window| {
                window.geometry = Some(geometry);
            });
        }
        Request::AckConfigure { serial } => {
            let acked = match object {
                None => return xdg_surface.post_error(Error::NotConstructed),
                Some(XdgObject::Toplevel(id)) => state
                    .shell()
                    .toplevels
                    .get_mut(id)
                    .is_none_or(|toplevel| toplevel.lifecycle.ack(serial)),
                Some(XdgObject::Popup(id)) => state
                    .shell()
                    .popups
                    .get_mut(id)
                    .is_none_or(|popup| popup.lifecycle.ack(serial)),
            };
            if !acked {
                xdg_surface.post_error(Error::InvalidSerial);
            }
        }
    }
}

pub(crate) fn destroyed<S: ShellHost>(state: &mut S, xdg_surface: XdgSurface) {
    if let Some(owner) = xdg_surface.take_key::<WmBaseId>()
        && let Some(record) = state.shell().wm_bases.get_mut(owner)
    {
        record.surfaces = record.surfaces.saturating_sub(1);
    }
    let Some(surface) = xdg_surface.key::<SurfaceId>() else {
        return;
    };
    let owns = state
        .surfaces()
        .role::<XdgRole>(surface)
        .is_some_and(|role| role.xdg_surface == xdg_surface);
    if !owns {
        return;
    }
    let Some(role) = state.surfaces().end_role::<XdgRole>(surface) else {
        return;
    };
    match role.object {
        Some(XdgObject::Toplevel(id)) => toplevel_lifecycle::remove(state, id),
        Some(XdgObject::Popup(id)) => popup_lifecycle::remove(state, id),
        None => {}
    }
}

fn edit_window<S: ShellHost>(
    state: &mut S,
    object: Option<XdgObject>,
    xdg_surface: &XdgSurface,
    edit: impl FnOnce(&mut super::window::WindowState),
) {
    let shell = state.shell();
    match object {
        None => xdg_surface.post_error(Error::NotConstructed),
        Some(XdgObject::Toplevel(id)) => {
            if let Some(toplevel) = shell.toplevels.get_mut(id) {
                edit(&mut toplevel.lifecycle.pending);
            }
        }
        Some(XdgObject::Popup(id)) => {
            if let Some(popup) = shell.popups.get_mut(id) {
                edit(&mut popup.lifecycle.pending);
            }
        }
    }
}
