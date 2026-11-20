use std::os::fd::OwnedFd;

use rikka_compositor_geometry::{Logical, Rect};
use rikka_compositor_selection::{MimeList, SelectionKind};
use rikka_compositor_xwm::{FrameExtents, Geometry, SelectionKind as XKind, XWindowId};
use rikka_types::DragAction;

use super::X11;
use crate::windows::Enlargements;

impl X11 {
    pub(crate) fn configure(&mut self, window: XWindowId, rect: Rect<i32, Logical>) -> bool {
        match self.command(|xwm| xwm.configure(window, geometry_of(rect))) {
            Some(Ok(_)) => true,
            Some(Err(error)) => {
                tracing::warn!("cannot configure X11 window: {error}");
                false
            }
            None => false,
        }
    }

    pub(crate) fn map(&mut self, window: XWindowId) -> bool {
        match self.command(|xwm| xwm.map(window)) {
            Some(Ok(())) => true,
            Some(Err(error)) => {
                tracing::warn!("cannot map X11 window: {error}");
                false
            }
            None => false,
        }
    }

    pub(crate) fn close(&mut self, window: XWindowId) {
        if let Some(Err(error)) = self.command(|xwm| xwm.close(window)) {
            tracing::warn!("cannot close X11 window: {error}");
        }
    }

    pub(crate) fn focus(&mut self, window: Option<XWindowId>) {
        if let Some(Err(error)) = self.command(|xwm| xwm.focus(window)) {
            tracing::debug!("cannot set X11 input focus: {error}");
        }
    }

    pub(crate) fn enlarge(&mut self, window: XWindowId, modes: Enlargements) {
        let result = self.command(|xwm| {
            xwm.set_maximized(window, modes.maximized)
                .and_then(|()| xwm.set_fullscreen(window, modes.fullscreen))
        });
        if let Some(Err(error)) = result {
            tracing::warn!("cannot update X11 window maximized state: {error}");
        }
    }

    pub(crate) fn minimize(&mut self, window: XWindowId, minimized: bool) {
        if let Some(Err(error)) = self.command(|xwm| xwm.set_minimized(window, minimized)) {
            tracing::debug!("cannot update X11 window minimized state: {error}");
        }
    }

    pub(crate) fn set_frame_extents(&mut self, window: XWindowId, extents: FrameExtents) {
        if let Some(Err(error)) = self.command(|xwm| xwm.set_frame_extents(window, extents)) {
            tracing::debug!("cannot publish X11 frame extents: {error}");
        }
    }

    pub(crate) fn restack(&mut self, bottom_to_top: &[XWindowId]) -> bool {
        match self.command(|xwm| xwm.restack(bottom_to_top.iter().copied())) {
            Some(Ok(())) => true,
            Some(Err(error)) => {
                tracing::warn!("cannot restack X11 windows: {error}");
                false
            }
            None => false,
        }
    }

    pub(crate) fn offer_selection(&mut self, kind: SelectionKind, offered: Option<MimeList>) {
        let result = self.command(|xwm| match offered {
            Some(mime_types) => xwm.set_selection(x11_kind(kind), mime_types),
            None => xwm.clear_selection(x11_kind(kind)),
        });
        if let Some(Err(error)) = result {
            tracing::warn!("cannot offer Wayland {kind} to X11: {error}");
        }
    }

    pub(crate) fn read_selection(&mut self, kind: SelectionKind, mime_type: &str, fd: OwnedFd) {
        if let Some(Err(error)) =
            self.command(|xwm| xwm.request_selection(x11_kind(kind), mime_type, fd))
        {
            tracing::warn!("cannot read X11 {kind}: {error}");
        }
    }

    pub(crate) fn read_drag(&mut self, mime_type: &str, fd: OwnedFd) {
        if let Some(Err(error)) =
            self.command(|xwm| xwm.request_selection(XKind::Drag, mime_type, fd))
        {
            tracing::debug!("cannot read X11 drag data: {error}");
        }
    }

    pub(crate) fn drag_enter(
        &mut self,
        window: XWindowId,
        mime_types: MimeList,
        at: (i32, i32),
        action: DragAction,
    ) {
        if let Some(Err(error)) = self.command(|xwm| xwm.drag_enter(window, mime_types, at, action))
        {
            tracing::debug!("cannot offer drag to X11 window: {error}");
        }
    }

    pub(crate) fn drag_motion(&mut self, at: (i32, i32), action: DragAction) {
        if let Some(Err(error)) = self.command(|xwm| xwm.drag_motion(at, action)) {
            tracing::debug!("cannot update X11 drag position: {error}");
        }
    }

    pub(crate) fn drag_leave(&mut self) {
        if let Some(Err(error)) = self.command(|xwm| xwm.drag_leave()) {
            tracing::debug!("cannot send X11 drag leave: {error}");
        }
    }

    pub(crate) fn drag_drop(&mut self) {
        if let Some(Err(error)) = self.command(|xwm| xwm.drag_drop()) {
            tracing::debug!("cannot send X11 drop: {error}");
        }
    }

    pub(crate) fn answer_drag(&mut self, accepted: bool, action: Option<DragAction>) {
        if let Some(Err(error)) = self.command(|xwm| xwm.answer_drag(accepted, action)) {
            tracing::debug!("cannot answer X11 drag: {error}");
        }
    }

    pub(crate) fn finish_drag(&mut self, accepted: bool, action: Option<DragAction>) {
        if let Some(Err(error)) = self.command(|xwm| xwm.finish_drag(accepted, action)) {
            tracing::debug!("cannot finish X11 drag: {error}");
        }
    }

    pub(crate) fn show_drop_proxy(&mut self, shown: bool) -> bool {
        match self.command(|xwm| xwm.show_drop_proxy(shown)) {
            Some(Ok(())) => true,
            Some(Err(error)) => {
                tracing::debug!("cannot place X11 drop proxy: {error}");
                false
            }
            None => false,
        }
    }
}

fn x11_kind(kind: SelectionKind) -> XKind {
    match kind {
        SelectionKind::Clipboard => XKind::Clipboard,
        SelectionKind::Primary => XKind::Primary,
    }
}

fn geometry_of(rect: Rect<i32, Logical>) -> Geometry {
    Geometry::new(
        rect.loc.x,
        rect.loc.y,
        rect.size.w.max(1).unsigned_abs(),
        rect.size.h.max(1).unsigned_abs(),
    )
}
