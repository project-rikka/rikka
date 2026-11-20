use rikka_compositor_geometry::{Point, Size};
use rikka_compositor_protocols::xdg_toplevel::{Error, Request, XdgToplevel};

use super::toplevel_lifecycle::{self, emit, toplevel_of};
use crate::event::ToplevelEvent;
use crate::host::ShellHost;
use crate::ids::ToplevelId;

pub(crate) fn request<S: ShellHost>(state: &mut S, toplevel: XdgToplevel, request: Request) {
    let Some(id) = toplevel_of(state, &toplevel) else {
        return;
    };
    match request {
        Request::Destroy => {
            if let Some(decoration) = state
                .shell()
                .toplevels
                .get(id)
                .and_then(|record| record.decoration.as_ref())
            {
                decoration.post_orphaned();
            }
        }
        Request::SetParent { parent } => {
            let parent = parent.and_then(|parent| toplevel_of(state, &parent));
            if !toplevel_lifecycle::set_parent(state, id, parent) {
                toplevel.post_error(Error::InvalidParent);
            }
        }
        Request::SetTitle { title } => {
            edit(state, id, |record| overwrite(&mut record.title, title));
            emit(state, id, ToplevelEvent::Title);
        }
        Request::SetAppId { app_id } => {
            edit(state, id, |record| overwrite(&mut record.app_id, app_id));
            emit(state, id, ToplevelEvent::AppId);
        }
        Request::ShowWindowMenu { seat, serial, x, y } => emit(
            state,
            id,
            ToplevelEvent::WindowMenu {
                seat,
                serial,
                at: Point::new(x, y),
            },
        ),
        Request::Move { seat, serial } => emit(state, id, ToplevelEvent::Move { seat, serial }),
        Request::Resize {
            seat,
            serial,
            edges,
        } => match edges {
            Ok(edges) => emit(
                state,
                id,
                ToplevelEvent::Resize {
                    seat,
                    serial,
                    edges,
                },
            ),
            Err(_) => toplevel.post_error(Error::InvalidResizeEdge),
        },
        Request::SetMaxSize { width, height } => {
            if width < 0 || height < 0 {
                return toplevel.post_error(Error::InvalidSize);
            }
            edit(state, id, |record| {
                record.lifecycle.pending.max_size = Size::new(width, height);
            });
        }
        Request::SetMinSize { width, height } => {
            if width < 0 || height < 0 {
                return toplevel.post_error(Error::InvalidSize);
            }
            edit(state, id, |record| {
                record.lifecycle.pending.min_size = Size::new(width, height);
            });
        }
        Request::SetMaximized => emit(state, id, ToplevelEvent::Maximize),
        Request::UnsetMaximized => emit(state, id, ToplevelEvent::Unmaximize),
        Request::SetFullscreen { output } => {
            emit(state, id, ToplevelEvent::Fullscreen { output });
        }
        Request::UnsetFullscreen => emit(state, id, ToplevelEvent::Unfullscreen),
        Request::SetMinimized => emit(state, id, ToplevelEvent::Minimize),
    }
}

pub(crate) fn destroyed<S: ShellHost>(state: &mut S, toplevel: XdgToplevel) {
    if let Some(id) = toplevel.key::<ToplevelId>() {
        toplevel_lifecycle::remove(state, id);
    }
}

fn edit<S: ShellHost>(state: &mut S, id: ToplevelId, change: impl FnOnce(&mut super::Toplevel)) {
    if let Some(record) = state.shell().toplevels.get_mut(id) {
        change(record);
    }
}

fn overwrite(slot: &mut Option<String>, text: &str) {
    match slot {
        Some(current) => {
            current.clear();
            current.push_str(text);
        }
        None => *slot = Some(text.to_owned()),
    }
}
