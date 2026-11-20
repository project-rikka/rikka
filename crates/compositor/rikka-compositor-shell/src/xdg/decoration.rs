use rikka_compositor_protocols::zxdg_decoration_manager_v1::{self, ZxdgDecorationManagerV1};
use rikka_compositor_protocols::zxdg_toplevel_decoration_v1::{
    self, Error, ZxdgToplevelDecorationV1,
};
use rikka_compositor_surface::Surface;

pub use rikka_compositor_protocols::zxdg_toplevel_decoration_v1::Mode as DecorationMode;

use super::toplevel_lifecycle::{emit, toplevel_of};
use crate::event::ToplevelEvent;
use crate::host::ShellHost;
use crate::ids::ToplevelId;

const BUFFER_BEFORE_DECORATION_SINCE: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecorationPreference {
    Absent,
    Unset,
    Prefers(DecorationMode),
}

#[derive(Debug)]
pub(crate) struct Decoration {
    resource: ZxdgToplevelDecorationV1,
    pub(crate) preference: DecorationPreference,
    configure_pending: bool,
    pub(crate) sent: Option<DecorationMode>,
}

impl Decoration {
    pub(crate) fn offer(&mut self, mode: DecorationMode) {
        if !self.configure_pending && self.sent == Some(mode) {
            return;
        }
        self.resource.configure(mode);
        self.sent = Some(mode);
        self.configure_pending = false;
    }

    pub(crate) fn post_orphaned(&self) {
        if self.resource.alive() {
            self.resource.post_error(Error::Orphaned);
        }
    }
}

pub(crate) fn manager<S: ShellHost>(
    state: &mut S,
    _manager: ZxdgDecorationManagerV1,
    request: zxdg_decoration_manager_v1::Request,
) {
    let zxdg_decoration_manager_v1::Request::GetToplevelDecoration { id, toplevel } = request
    else {
        return;
    };
    let Some(toplevel_id) = toplevel_of(state, &toplevel) else {
        return;
    };
    let shell = state.shell();
    let Some(record) = shell.toplevels.get(toplevel_id) else {
        return;
    };
    if record
        .decoration
        .as_ref()
        .is_some_and(|decoration| decoration.resource.alive())
    {
        return id.post_error(Error::AlreadyConstructed);
    }
    let surface = record.surface;
    if id.version() < BUFFER_BEFORE_DECORATION_SINCE
        && state
            .surfaces()
            .get(surface)
            .is_some_and(Surface::buffer_attached)
    {
        return id.post_error(Error::UnconfiguredBuffer);
    }
    id.set_key(toplevel_id);
    if let Some(record) = state.shell().toplevels.get_mut(toplevel_id) {
        record.decoration = Some(Decoration {
            resource: id,
            preference: DecorationPreference::Unset,
            configure_pending: true,
            sent: None,
        });
    }
    notify(state, toplevel_id);
}

pub(crate) fn request<S: ShellHost>(
    state: &mut S,
    decoration: ZxdgToplevelDecorationV1,
    request: zxdg_toplevel_decoration_v1::Request,
) {
    let preference = match request {
        zxdg_toplevel_decoration_v1::Request::Destroy => return,
        zxdg_toplevel_decoration_v1::Request::SetMode { mode: Ok(mode) } => {
            DecorationPreference::Prefers(mode)
        }
        zxdg_toplevel_decoration_v1::Request::SetMode { mode: Err(_) } => {
            return decoration.post_error(Error::InvalidMode);
        }
        zxdg_toplevel_decoration_v1::Request::UnsetMode => DecorationPreference::Unset,
    };
    let Some(toplevel) = decoration.key::<ToplevelId>() else {
        return;
    };
    let Some(record) = state
        .shell()
        .toplevels
        .get_mut(toplevel)
        .and_then(|record| record.decoration.as_mut())
        .filter(|record| record.resource == decoration)
    else {
        return;
    };
    record.preference = preference;
    record.configure_pending = true;
    notify(state, toplevel);
}

pub(crate) fn destroyed<S: ShellHost>(state: &mut S, decoration: ZxdgToplevelDecorationV1) {
    let Some(toplevel) = decoration.key::<ToplevelId>() else {
        return;
    };
    let Some(record) = state.shell().toplevels.get_mut(toplevel) else {
        return;
    };
    if record
        .decoration
        .as_ref()
        .is_some_and(|current| current.resource == decoration)
    {
        record.decoration = None;
        notify(state, toplevel);
    }
}

fn notify<S: ShellHost>(state: &mut S, toplevel: ToplevelId) {
    emit(state, toplevel, ToplevelEvent::Decoration);
}
