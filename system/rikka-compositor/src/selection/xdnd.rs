use std::os::fd::OwnedFd;
use std::time::Duration;

use rikka_compositor_desktop::WindowKind;
use rikka_compositor_protocols::wl_data_device_manager::DndAction;
use rikka_compositor_seat::Focus;
use rikka_compositor_selection::{
    CompositorDragEvent, CompositorSource, ForeignDragEvent, ForeignOffer, MimeList, SendDrag,
    cancel_compositor_drag, drop_compositor_drag, foreign_drag_data, foreign_drag_status,
    foreign_drop_finished, move_compositor_drag, start_compositor_drag,
};
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_xwm::XWindowId;
use rikka_types::DragAction;

use crate::context::WindowsCtx;
use crate::context::{InputCtx, input_ref, selection_parts};
use crate::render::monotonic;
use crate::selection::Selection;
use crate::windows::Windows;

#[derive(Debug, Default)]
pub(crate) struct XDrag {
    toward: Option<DropTarget>,
    landing: Option<ForeignOffer>,
    owned: bool,
    proxy: bool,
    from: Option<FromX11>,
}

#[derive(Debug, Clone, Copy)]
struct DropTarget {
    offer: ForeignOffer,
    action: DragAction,
}

#[derive(Debug, Clone, Copy)]
struct FromX11 {
    source: CompositorSource,
    dropping: bool,
    accepted: bool,
    action: Option<DragAction>,
}

impl FromX11 {
    fn new(source: CompositorSource) -> Self {
        Self {
            source,
            dropping: false,
            accepted: false,
            action: None,
        }
    }
}

fn preferred(actions: DndAction) -> DragAction {
    if actions.contains(DndAction::COPY) {
        DragAction::Copy
    } else if actions.contains(DndAction::MOVE) {
        DragAction::Move
    } else {
        DragAction::Ask
    }
}

fn wayland_action(action: DragAction) -> DndAction {
    match action {
        DragAction::Copy => DndAction::COPY,
        DragAction::Move => DndAction::MOVE,
        DragAction::Ask => DndAction::ASK,
    }
}

fn x11_action(action: DndAction) -> Option<DragAction> {
    [
        (DndAction::COPY, DragAction::Copy),
        (DndAction::MOVE, DragAction::Move),
        (DndAction::ASK, DragAction::Ask),
    ]
    .into_iter()
    .find(|(wire, _)| action == *wire)
    .map(|(_, action)| action)
}

impl Windows {
    pub(crate) fn x11_window_of(&self, surface: SurfaceId) -> Option<XWindowId> {
        self.desktop
            .windows()
            .find(|(_, window)| window.surface() == Some(surface))
            .and_then(|(_, window)| match window.kind() {
                WindowKind::X11(x11) => Some(x11),
                WindowKind::Xdg(_) => None,
            })
    }
}

impl InputCtx<'_> {
    pub(super) fn compositor_dragged(
        &mut self,
        source: CompositorSource,
        event: CompositorDragEvent,
    ) {
        let Some(from) = self
            .selection
            .xdrag
            .from
            .as_mut()
            .filter(|from| from.source == source)
        else {
            return;
        };
        match event {
            CompositorDragEvent::Target(accepted) => from.accepted = accepted,
            CompositorDragEvent::Action(action) => from.action = x11_action(action),
            CompositorDragEvent::Finished(action) => {
                let action = action.and_then(x11_action).or(from.action);
                self.selection.xdrag.from = None;
                return self.x11.finish_drag(true, action);
            }
            CompositorDragEvent::Cancelled => {
                let dropping = from.dropping;
                self.selection.xdrag.from = None;
                if dropping {
                    self.x11.finish_drag(false, None);
                }
                return;
            }
        }
        let (accepted, action) = (from.accepted && from.action.is_some(), from.action);
        self.x11.answer_drag(accepted, action);
    }

    pub(super) fn x11_drag_request(&mut self, request: SendDrag) {
        if self.selection.x11_source() != Some(request.source) {
            return;
        }
        self.x11.read_drag(&request.mime_type, request.fd);
    }
}

impl Selection {
    fn x11_source(&self) -> Option<CompositorSource> {
        self.xdrag.from.map(|from| from.source)
    }
}

impl WindowsCtx<'_> {
    pub(super) fn foreign_dragged(&mut self, offer: ForeignOffer, event: ForeignDragEvent) {
        let toward = self
            .selection
            .xdrag
            .toward
            .filter(|toward| toward.offer == offer);
        match event {
            ForeignDragEvent::Entered { surface, at } => {
                let Some(window) = self.windows.x11_window_of(surface) else {
                    return;
                };
                let Some((mime_types, actions)) = self
                    .selection
                    .selections
                    .foreign_offer(offer)
                    .map(|info| (MimeList::clone(info.mime_types), info.actions))
                else {
                    return;
                };
                self.abandon_landing();
                let action = preferred(actions);
                self.selection.xdrag.toward = Some(DropTarget { offer, action });
                let at = (at.x.round() as i32, at.y.round() as i32);
                self.x11.drag_enter(window, mime_types, at, action);
            }
            ForeignDragEvent::Moved { at } => {
                let Some(toward) = toward else {
                    return;
                };
                let at = (at.x.round() as i32, at.y.round() as i32);
                self.x11.drag_motion(at, toward.action);
            }
            ForeignDragEvent::Left => {
                if toward.is_none() {
                    return;
                }
                self.selection.xdrag.toward = None;
                self.x11.drag_leave();
            }
            ForeignDragEvent::Dropped => {
                if toward.is_none() {
                    return;
                }
                self.selection.xdrag.toward = None;
                self.abandon_landing();
                self.selection.xdrag.landing = Some(offer);
                self.x11.drag_drop();
            }
        }
    }

    pub(crate) fn x11_drag_owned(&mut self, owned: bool) {
        self.selection.xdrag.owned = owned;
        if !owned {
            self.x11_drag_follows(monotonic());
        }
    }

    pub(crate) fn x11_drag_entered(&mut self, mime_types: MimeList) {
        let seat = self.input.seat;
        if let Some(from) = self.selection.xdrag.from.take() {
            tracing::debug!("X11 drag re-entered before leaving {:?}", from.source);
            cancel_compositor_drag(&mut selection_parts!(self), seat);
        }
        let at = self.cursor.position();
        match start_compositor_drag(
            &mut selection_parts!(self),
            seat,
            mime_types,
            DndAction::COPY | DndAction::MOVE,
            at,
        ) {
            Ok(source) => {
                self.selection.xdrag.from = Some(FromX11::new(source));
                self.x11_drag_follows(monotonic());
            }
            Err(error) => tracing::debug!("cannot bridge X11 drag to Wayland: {error}"),
        }
    }

    pub(crate) fn x11_drag_left(&mut self) {
        if self.selection.xdrag.from.take().is_some() {
            let seat = self.input.seat;
            cancel_compositor_drag(&mut selection_parts!(self), seat);
        }
    }

    pub(crate) fn x11_drag_dropped(&mut self) {
        let Some(from) = self.selection.xdrag.from.as_mut() else {
            return self.x11.finish_drag(false, None);
        };
        from.dropping = true;
        let seat = self.input.seat;
        drop_compositor_drag(&mut selection_parts!(self), seat);
    }

    pub(crate) fn x11_drag_status(&mut self, accepted: bool, action: Option<DragAction>) {
        let Some(toward) = self.selection.xdrag.toward else {
            return;
        };
        let action = action.map_or(DndAction::empty(), wayland_action);
        foreign_drag_status(&mut selection_parts!(self), toward.offer, accepted, action);
    }

    pub(crate) fn x11_drag_finished(&mut self, accepted: bool, action: Option<DragAction>) {
        if let Some(offer) = self.selection.xdrag.landing.take() {
            foreign_drop_finished(
                &mut selection_parts!(self),
                offer,
                accepted,
                action.map(wayland_action),
            );
        }
    }

    pub(crate) fn x11_drag_requested(&mut self, mime_type: String, fd: OwnedFd) {
        let xdrag = &self.selection.xdrag;
        let Some(offer) = xdrag.toward.map(|toward| toward.offer).or(xdrag.landing) else {
            return;
        };
        if let Err(error) = foreign_drag_data(&mut selection_parts!(self), offer, mime_type, fd) {
            tracing::debug!("X11 window requested a finished drag: {error}");
        }
    }

    pub(crate) fn x11_drag_follows(&mut self, time: Duration) {
        let xdrag = &self.selection.xdrag;
        if !xdrag.owned && !xdrag.proxy && xdrag.from.is_none() {
            return;
        }
        let (button_pressed, grabbed) = self
            .seats
            .get(self.input.seat)
            .map(|entry| {
                let pointer = entry.pointer();
                (!pointer.buttons().is_empty(), pointer.focus())
            })
            .unwrap_or_default();
        let at = self.cursor.position();
        let under = input_ref!(self).element_under(at);
        let over_x11 = under.is_some_and(|under| self.windows.is_x11_target(under.target));
        let shown = self.selection.xdrag.owned
            && button_pressed
            && !over_x11
            && grabbed.is_some_and(|focus| self.windows.x11_window_of(focus.surface).is_some());
        if self.selection.xdrag.proxy != shown && self.x11.show_drop_proxy(shown) {
            self.selection.xdrag.proxy = shown;
        }
        if self.selection.xdrag.from.is_some() {
            let focus = under
                .filter(|_| !over_x11)
                .map(|under| Focus::new(under.surface, at - under.local));
            let seat = self.input.seat;
            move_compositor_drag(&mut selection_parts!(self), seat, at, focus, time);
        }
    }

    pub(crate) fn x11_drags_gone(&mut self) {
        let from = self.selection.xdrag.from.is_some();
        self.abandon_landing();
        self.selection.xdrag = XDrag::default();
        if from {
            let seat = self.input.seat;
            cancel_compositor_drag(&mut selection_parts!(self), seat);
        }
    }

    fn abandon_landing(&mut self) {
        if let Some(offer) = self.selection.xdrag.landing.take() {
            foreign_drop_finished(&mut selection_parts!(self), offer, false, None);
        }
    }
}
