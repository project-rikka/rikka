use rikka_compositor_protocols::xdg_toplevel_tag_manager_v1::{Request, XdgToplevelTagManagerV1};

use super::toplevel_lifecycle::{emit, toplevel_of};
use crate::event::ToplevelEvent;
use crate::host::ShellHost;

pub(crate) fn request<S: ShellHost>(
    state: &mut S,
    _manager: XdgToplevelTagManagerV1,
    request: Request,
) {
    match request {
        Request::Destroy => {}
        Request::SetToplevelTag { toplevel, tag } => {
            let Some(id) = toplevel_of(state, &toplevel) else {
                return;
            };
            if let Some(record) = state.shell().toplevels.get_mut(id) {
                record.tag = Some(tag.to_owned());
            }
            emit(state, id, ToplevelEvent::Tag);
        }
        Request::SetToplevelDescription {
            toplevel,
            description,
        } => {
            let Some(id) = toplevel_of(state, &toplevel) else {
                return;
            };
            if let Some(record) = state.shell().toplevels.get_mut(id) {
                record.description = Some(description.to_owned());
            }
            emit(state, id, ToplevelEvent::Description);
        }
    }
}
