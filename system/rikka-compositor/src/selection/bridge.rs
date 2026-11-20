use std::os::fd::OwnedFd;

use rikka_compositor_seat::SeatId;
use rikka_compositor_selection::{
    CompositorSource, MimeList, SelectionKind, SendSelection, Source, clear_selection,
    request_data, set_compositor_selection,
};
use rikka_compositor_xwm::SelectionKind as XKind;

use crate::context::WindowsCtx;
use crate::context::{InputCtx, selection_parts};
use crate::selection::Selection;

const KINDS: [SelectionKind; 2] = [SelectionKind::Clipboard, SelectionKind::Primary];

#[derive(Debug, Default)]
pub(crate) struct XBridge {
    clipboard: Option<CompositorSource>,
    primary: Option<CompositorSource>,
    adopting: bool,
}

impl XBridge {
    fn own(&self, kind: SelectionKind) -> Option<CompositorSource> {
        match kind {
            SelectionKind::Clipboard => self.clipboard,
            SelectionKind::Primary => self.primary,
        }
    }

    fn own_mut(&mut self, kind: SelectionKind) -> &mut Option<CompositorSource> {
        match kind {
            SelectionKind::Clipboard => &mut self.clipboard,
            SelectionKind::Primary => &mut self.primary,
        }
    }
}

fn wayland_kind(kind: XKind) -> Option<SelectionKind> {
    match kind {
        XKind::Clipboard => Some(SelectionKind::Clipboard),
        XKind::Primary => Some(SelectionKind::Primary),
        XKind::Drag => None,
    }
}

impl InputCtx<'_> {
    pub(super) fn x11_send_selection(&mut self, request: SendSelection) {
        if self.selection.bridge.own(request.kind) != Some(request.source) {
            return;
        }
        self.x11
            .read_selection(request.kind, &request.mime_type, request.fd);
    }

    pub(super) fn selection_changed(&mut self, seat: SeatId, kind: SelectionKind) {
        if seat != self.input.seat || self.selection.bridge.adopting {
            return;
        }
        if let Some(own) = self.selection.bridge.own(kind) {
            if self.selection.bridge_owns_selection(seat, kind, own) {
                return;
            }
            *self.selection.bridge.own_mut(kind) = None;
        }
        let offered = self
            .selection
            .selections
            .current(seat, kind)
            .map(|current| MimeList::clone(current.mime_types));
        self.x11.offer_selection(kind, offered);
    }

    pub(crate) fn bridge_selections_to_x11(&mut self) {
        let seat = self.input.seat;
        for kind in KINDS {
            self.selection_changed(seat, kind);
        }
    }
}

impl Selection {
    fn bridge_owns_selection(
        &self,
        seat: SeatId,
        kind: SelectionKind,
        own: CompositorSource,
    ) -> bool {
        self.selections
            .current(seat, kind)
            .is_some_and(|current| current.source == Source::Compositor(own))
    }
}

impl WindowsCtx<'_> {
    pub(crate) fn x11_selection_offered(&mut self, kind: XKind, mime_types: MimeList) {
        let Some(kind) = wayland_kind(kind) else {
            return;
        };
        let seat = self.input.seat;
        self.selection.bridge.adopting = true;
        let adopted = set_compositor_selection(&mut selection_parts!(self), seat, kind, mime_types);
        self.selection.bridge.adopting = false;
        match adopted {
            Ok(source) => *self.selection.bridge.own_mut(kind) = Some(source),
            Err(error) => tracing::warn!("cannot offer X11 {kind} to Wayland: {error}"),
        }
    }

    pub(crate) fn x11_selection_cleared(&mut self, kind: XKind) {
        let Some(kind) = wayland_kind(kind) else {
            return;
        };
        let seat = self.input.seat;
        let Some(own) = self.selection.bridge.own_mut(kind).take() else {
            return;
        };
        if self.selection.bridge_owns_selection(seat, kind, own) {
            clear_selection(&mut selection_parts!(self), seat, kind);
        }
    }

    pub(crate) fn x11_selection_requested(&mut self, kind: XKind, mime_type: String, fd: OwnedFd) {
        let Some(kind) = wayland_kind(kind) else {
            return self.x11_drag_requested(mime_type, fd);
        };
        let seat = self.input.seat;
        if let Err(error) = request_data(&mut selection_parts!(self), seat, kind, mime_type, fd) {
            tracing::debug!("X11 client requested an unavailable {kind}: {error}");
        }
    }
}
