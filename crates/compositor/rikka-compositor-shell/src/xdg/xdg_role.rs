use rikka_compositor_protocols::xdg_surface::{self, XdgSurface};
use rikka_compositor_protocols::xdg_toplevel;
use rikka_compositor_protocols::xdg_wm_base::{self, XdgWmBase};
use rikka_compositor_surface::{
    BufferChange, Committed, PreCommit, Rejected, Role, Surface, SurfaceId,
};

use super::{popup_lifecycle, toplevel_lifecycle};
use crate::host::ShellHost;
use crate::ids::{PopupId, ToplevelId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum XdgObject {
    Toplevel(ToplevelId),
    Popup(PopupId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum XdgKind {
    Toplevel,
    Popup,
}

impl rikka_wayland_wire::Key for XdgKind {
    fn into_raw(self) -> u64 {
        match self {
            Self::Toplevel => 0,
            Self::Popup => 1,
        }
    }

    fn from_raw(raw: u64) -> Self {
        match raw {
            0 => Self::Toplevel,
            _ => Self::Popup,
        }
    }
}

#[derive(Debug)]
pub(crate) struct XdgRole {
    pub(crate) xdg_surface: XdgSurface,
    pub(crate) wm_base: XdgWmBase,
    pub(crate) object: Option<XdgObject>,
    pub(crate) constructed: bool,
}

#[derive(Debug, Clone, Copy)]
enum Fault {
    Surface(xdg_surface::Error),
    WmBase(xdg_wm_base::Error),
    Toplevel(ToplevelId, xdg_toplevel::Error),
}

impl<S: ShellHost> Role<S> for XdgRole {
    const NAME: &'static str = "xdg_surface";

    fn pre_commit(state: &mut S, commit: &PreCommit) -> Result<(), Rejected> {
        let Some(role) = state.surfaces().role::<XdgRole>(commit.surface) else {
            return Ok(());
        };
        let object = role.object;
        let constructed = role.constructed;
        let attaches = commit.buffer == BufferChange::Attached;
        let shell = state.shell();
        let fault = match object {
            None if attaches => Some(Fault::Surface(xdg_surface::Error::UnconfiguredBuffer)),
            None if constructed => Some(Fault::Surface(xdg_surface::Error::NotConstructed)),
            None => None,
            Some(XdgObject::Toplevel(id)) => {
                let Some(toplevel) = shell.toplevels.get_mut(id) else {
                    return Ok(());
                };
                if attaches && !toplevel.lifecycle.configured() {
                    Some(Fault::Surface(xdg_surface::Error::UnconfiguredBuffer))
                } else if !toplevel.lifecycle.pending.limits_valid() {
                    Some(Fault::Toplevel(id, xdg_toplevel::Error::InvalidSize))
                } else {
                    toplevel.lifecycle.latch();
                    None
                }
            }
            Some(XdgObject::Popup(id)) => {
                let Some(popup) = shell.popups.get_mut(id) else {
                    return Ok(());
                };
                if popup.parent.is_none() {
                    Some(Fault::WmBase(xdg_wm_base::Error::InvalidPopupParent))
                } else if attaches && !popup.lifecycle.configured() {
                    Some(Fault::Surface(xdg_surface::Error::UnconfiguredBuffer))
                } else {
                    popup.lifecycle.latch();
                    None
                }
            }
        };
        match fault {
            Some(fault) => {
                post(state, commit.surface, fault);
                Err(Rejected)
            }
            None => Ok(()),
        }
    }

    fn committed(state: &mut S, commit: &Committed) {
        let Some(object) = state
            .surfaces()
            .role::<XdgRole>(commit.surface)
            .and_then(|role| role.object)
        else {
            return;
        };
        let has_buffer = state
            .surfaces()
            .get(commit.surface)
            .is_some_and(Surface::mapped);
        match object {
            XdgObject::Toplevel(id) => toplevel_lifecycle::committed(state, id, has_buffer),
            XdgObject::Popup(id) => popup_lifecycle::committed(state, id, has_buffer),
        }
    }

    fn destroyed(state: &mut S, _surface: SurfaceId, role: Self) {
        match role.object {
            Some(XdgObject::Toplevel(id)) => toplevel_lifecycle::remove(state, id),
            Some(XdgObject::Popup(id)) => popup_lifecycle::remove(state, id),
            None => {}
        }
    }
}

fn post<S: ShellHost>(state: &mut S, surface: SurfaceId, fault: Fault) {
    match fault {
        Fault::Surface(error) => {
            if let Some(role) = state.surfaces().role::<XdgRole>(surface) {
                role.xdg_surface.post_error(error);
            }
        }
        Fault::WmBase(error) => {
            if let Some(role) = state.surfaces().role::<XdgRole>(surface) {
                role.wm_base.post_error(error);
            }
        }
        Fault::Toplevel(toplevel, error) => {
            if let Some(record) = state.shell().toplevels.get(toplevel) {
                record.resource.post_error(error);
            }
        }
    }
}
