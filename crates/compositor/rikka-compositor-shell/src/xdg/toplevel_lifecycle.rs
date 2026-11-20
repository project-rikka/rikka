use rikka_compositor_protocols::xdg_surface::{self, XdgSurface};
use rikka_compositor_protocols::xdg_toplevel::XdgToplevel;
use rikka_compositor_protocols::xdg_wm_base::{self, XdgWmBase};
use rikka_compositor_surface::SurfaceId;

use super::popup::PopupParent;
use super::toplevel::Toplevel;
use super::window::WindowState;
use super::xdg_role::{XdgKind, XdgObject, XdgRole};
use super::{drag, foreign};
use crate::event::{ShellEvent, ToplevelEvent};
use crate::host::ShellHost;
use crate::ids::ToplevelId;
use crate::lifecycle::Transition;
use crate::shell::Shell;

pub(crate) fn toplevel_of<S: ShellHost>(
    state: &mut S,
    toplevel: &XdgToplevel,
) -> Option<ToplevelId> {
    toplevel
        .key::<ToplevelId>()
        .filter(|id| state.shell().toplevels.contains_key(*id))
}

pub(crate) fn create<S: ShellHost>(
    state: &mut S,
    surface: SurfaceId,
    xdg_surface: XdgSurface,
    wm_base: XdgWmBase,
    resource: XdgToplevel,
) {
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
    if wl_surface.key::<XdgKind>() == Some(XdgKind::Popup) {
        return wm_base.post_error(xdg_wm_base::Error::Role);
    }
    wl_surface.set_key(XdgKind::Toplevel);
    let id = state
        .shell()
        .toplevels
        .insert(Toplevel::new(resource.clone(), xdg_surface, surface));
    resource.set_key(id);
    if let Some(role) = state.surfaces().role_mut::<XdgRole>(surface) {
        role.object = Some(XdgObject::Toplevel(id));
        role.constructed = true;
    }
    emit(state, id, ToplevelEvent::Created);
}

pub(crate) fn remove<S: ShellHost>(state: &mut S, id: ToplevelId) {
    let Some(record) = state.shell().toplevels.remove(id) else {
        return;
    };
    if let Some(role) = state.surfaces().role_mut::<XdgRole>(record.surface)
        && role.object == Some(XdgObject::Toplevel(id))
    {
        role.object = None;
    }
    adopt_children(state, id, record.parent);
    state.shell().dismiss_children(PopupParent::Toplevel(id));
    drag::detach(state.shell(), id);
    foreign::toplevel_gone(state, id);
    emit(state, id, ToplevelEvent::Destroyed);
}

pub(crate) fn committed<S: ShellHost>(state: &mut S, id: ToplevelId, has_buffer: bool) {
    let Some(record) = state.shell().toplevels.get_mut(id) else {
        return;
    };
    let applied = record.lifecycle.apply(has_buffer, &WindowState::default());
    let parent = record.parent;
    if applied.transition == Some(Transition::Unmapped) {
        record.forget_attributes();
    }
    if let Some(serial) = applied.configured {
        emit(state, id, ToplevelEvent::Configured { serial });
    }
    match applied.transition {
        Some(Transition::InitialCommit) => emit(state, id, ToplevelEvent::InitialCommit),
        Some(Transition::Mapped) => emit(state, id, ToplevelEvent::Mapped),
        Some(Transition::Unmapped) => {
            adopt_children(state, id, parent);
            drag::detach(state.shell(), id);
            emit(state, id, ToplevelEvent::Unmapped);
        }
        None => {}
    }
}

pub(crate) fn set_parent<S: ShellHost>(
    state: &mut S,
    child: ToplevelId,
    parent: Option<ToplevelId>,
) -> bool {
    let shell = state.shell();
    if creates_loop(shell, child, parent) {
        return false;
    }
    let Some(record) = shell.toplevels.get_mut(child) else {
        return true;
    };
    if record.parent != parent {
        record.parent = parent;
        emit(state, child, ToplevelEvent::Parent);
    }
    true
}

fn creates_loop(shell: &Shell, child: ToplevelId, parent: Option<ToplevelId>) -> bool {
    let mut current = parent;
    while let Some(ancestor) = current {
        if ancestor == child {
            return true;
        }
        current = shell
            .toplevels
            .get(ancestor)
            .and_then(|toplevel| toplevel.parent);
    }
    false
}

fn adopt_children<S: ShellHost>(state: &mut S, id: ToplevelId, parent: Option<ToplevelId>) {
    let children: Vec<ToplevelId> = state
        .shell()
        .toplevels
        .iter()
        .filter(|(_, toplevel)| toplevel.parent == Some(id))
        .map(|(child, _)| child)
        .collect();
    for child in children {
        if let Some(record) = state.shell().toplevels.get_mut(child) {
            record.parent = parent;
        }
        emit(state, child, ToplevelEvent::Parent);
    }
}

pub(crate) fn emit<S: ShellHost>(state: &mut S, toplevel: ToplevelId, event: ToplevelEvent) {
    state.shell_event(ShellEvent::Toplevel { toplevel, event });
}
