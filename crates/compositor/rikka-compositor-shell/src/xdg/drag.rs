use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_protocols::wl_data_source::WlDataSource;
use rikka_compositor_protocols::xdg_toplevel_drag_manager_v1::{self, XdgToplevelDragManagerV1};
use rikka_compositor_protocols::xdg_toplevel_drag_v1::{self, Error, XdgToplevelDragV1};

use super::toplevel_lifecycle::{emit, toplevel_of};
use crate::event::ToplevelEvent;
use crate::host::ShellHost;
use crate::ids::{ToplevelDragId, ToplevelId};
use crate::shell::Shell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToplevelDrag {
    pub toplevel: Option<ToplevelId>,
    pub offset: Point<i32, Logical>,
}

#[derive(Debug)]
pub(crate) struct Drag {
    source: WlDataSource,
    attached: ToplevelDrag,
    ongoing: bool,
}

impl Shell {
    pub fn toplevel_drag(&self, source: &WlDataSource) -> Option<ToplevelDrag> {
        self.drags
            .values()
            .find(|drag| drag.source == *source)
            .map(|drag| drag.attached)
    }

    pub fn dragged_toplevel_offset(&self, toplevel: ToplevelId) -> Option<Point<i32, Logical>> {
        self.drags
            .values()
            .find(|drag| drag.ongoing && drag.attached.toplevel == Some(toplevel))
            .map(|drag| drag.attached.offset)
    }

    pub fn toplevel_drag_started(&mut self, source: &WlDataSource) -> bool {
        self.mark_drag(source, true)
    }

    pub fn toplevel_drag_ended(&mut self, source: &WlDataSource) {
        self.mark_drag(source, false);
    }

    fn mark_drag(&mut self, source: &WlDataSource, ongoing: bool) -> bool {
        let Some(drag) = self.drags.values_mut().find(|drag| drag.source == *source) else {
            return false;
        };
        drag.ongoing = ongoing;
        true
    }
}

pub(crate) fn detach(shell: &mut Shell, toplevel: ToplevelId) {
    for drag in shell.drags.values_mut() {
        if drag.attached.toplevel == Some(toplevel) {
            drag.attached.toplevel = None;
        }
    }
}

pub(crate) fn manager<S: ShellHost>(
    state: &mut S,
    manager: XdgToplevelDragManagerV1,
    request: xdg_toplevel_drag_manager_v1::Request,
) {
    let xdg_toplevel_drag_manager_v1::Request::GetXdgToplevelDrag { id, data_source } = request
    else {
        return;
    };
    let shell = state.shell();
    if shell.drags.values().any(|drag| drag.source == data_source) {
        return manager.post_error(xdg_toplevel_drag_manager_v1::Error::InvalidSource);
    }
    let key = shell.drags.insert(Drag {
        source: data_source,
        attached: ToplevelDrag {
            toplevel: None,
            offset: Point::zero(),
        },
        ongoing: false,
    });
    id.set_key(key);
}

pub(crate) fn request<S: ShellHost>(
    state: &mut S,
    drag: XdgToplevelDragV1,
    request: xdg_toplevel_drag_v1::Request,
) {
    let Some(key) = drag.key::<ToplevelDragId>() else {
        return;
    };
    match request {
        xdg_toplevel_drag_v1::Request::Destroy => {
            if state
                .shell()
                .drags
                .get(key)
                .is_some_and(|record| record.ongoing)
            {
                drag.post_error(Error::OngoingDrag);
            }
        }
        xdg_toplevel_drag_v1::Request::Attach {
            toplevel,
            x_offset,
            y_offset,
        } => {
            let toplevel = toplevel_of(state, &toplevel);
            let shell = state.shell();
            let Some(record) = shell.drags.get(key) else {
                return;
            };
            let attached = record
                .attached
                .toplevel
                .filter(|current| shell.toplevels.contains_key(*current));
            if attached.is_some() {
                return drag.post_error(Error::ToplevelAttached);
            }
            let Some(toplevel) = toplevel else {
                return;
            };
            let offset = Point::new(x_offset, y_offset);
            let Some(record) = state.shell().drags.get_mut(key) else {
                return;
            };
            record.attached = ToplevelDrag {
                toplevel: Some(toplevel),
                offset,
            };
            let source = record.source.clone();
            emit(
                state,
                toplevel,
                ToplevelEvent::DragAttached { source, offset },
            );
        }
    }
}

pub(crate) fn destroyed<S: ShellHost>(state: &mut S, drag: XdgToplevelDragV1) {
    if let Some(key) = drag.key::<ToplevelDragId>() {
        state.shell().drags.remove(key);
    }
}
