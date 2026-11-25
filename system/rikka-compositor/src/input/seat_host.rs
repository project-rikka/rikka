use rikka_compositor_input::PointerRestrictions;
use rikka_compositor_seat::{
    ConstraintKind, ConstraintState, CursorImage, KeyFilter, Keystroke, SeatEvent, SeatHost,
    SeatId, Seats,
};
use rikka_compositor_server::DisplayHandle;
use rikka_session::pointer::PointerMode;

use crate::context::InputCtx;
use crate::context::{input_ctx, parts, switcher_ctx, windows_ctx};
use crate::input::modifiers;
use crate::state::Compositor;

impl SeatHost for Compositor {
    fn seats(&mut self) -> &mut Seats {
        &mut self.seats
    }

    fn display(&self) -> &DisplayHandle<Self> {
        &self.core.display
    }

    fn filter_key(&mut self, keystroke: &Keystroke) -> KeyFilter {
        match keystroke.seat == self.input.agent.seat() {
            true => self.agent_keystroke(keystroke),
            false => {
                input_ctx!(self).person_active();
                windows_ctx!(self).filter(keystroke)
            }
        }
    }

    fn seat_event(&mut self, event: SeatEvent) {
        rikka_compositor_selection::seat_event(self, &event);
        match event {
            SeatEvent::VirtualPointer {
                seat,
                output,
                input,
            } => self.virtual_pointer(seat, output.as_ref(), input),
            SeatEvent::IdleInhibitors => switcher_ctx!(self).judge_idle_inhibitors(),
            SeatEvent::Warp { seat, location } if seat == self.input.agent.seat() => {
                input_ctx!(self).agent_warped(location);
            }
            event if event_seat(&event) == Some(self.input.seat) => {
                input_ctx!(self).person_event(event)
            }
            _ => {}
        }
    }
}

fn event_seat(event: &SeatEvent) -> Option<SeatId> {
    match event {
        SeatEvent::KeyboardFocus { seat, .. }
        | SeatEvent::KeyboardPresented { seat }
        | SeatEvent::KeyboardGrab { seat, .. }
        | SeatEvent::TouchGrab { seat, .. }
        | SeatEvent::PointerGrab { seat, .. }
        | SeatEvent::Cursor { seat, .. }
        | SeatEvent::ToolCursor { seat, .. }
        | SeatEvent::Constraint { seat, .. }
        | SeatEvent::CursorHint { seat, .. }
        | SeatEvent::Warp { seat, .. }
        | SeatEvent::Caret { seat, .. }
        | SeatEvent::InputPanel { seat, .. }
        | SeatEvent::InputPopup { seat, .. }
        | SeatEvent::InputPopupGone { seat, .. }
        | SeatEvent::VirtualPointer { seat, .. }
        | SeatEvent::TextInput { seat, .. }
        | SeatEvent::Modifiers { seat, .. } => Some(*seat),
        SeatEvent::IdleInhibitors => None,
    }
}

impl InputCtx<'_> {
    fn person_event(&mut self, event: SeatEvent) {
        match event {
            SeatEvent::KeyboardFocus { focus, .. } => {
                switcher_ctx!(self).set_pad_focus(focus.map(|focus| focus.surface()));
                windows_ctx!(self).refresh_activation_states();
            }
            SeatEvent::Cursor { image, .. } if self.input.tablets.proximity_tool().is_none() => {
                self.cursor_shown(image);
            }
            SeatEvent::ToolCursor { tool, image, .. }
                if self.input.tablets.proximity_tool() == Some(tool) =>
            {
                self.cursor_shown(image);
            }
            SeatEvent::PointerGrab { grabbed, .. } => {
                self.restrain(PointerRestrictions::GRABBED, grabbed);
            }
            SeatEvent::Constraint { state, .. } => {
                self.restrain(
                    PointerRestrictions::CONSTRAINED,
                    state != ConstraintState::Released,
                );
                self.announce_pointer_mode();
            }
            SeatEvent::CursorHint { location, .. } | SeatEvent::Warp { location, .. } => {
                self.warp_pointer(location);
            }
            SeatEvent::Caret { caret, .. } => {
                self.input.caret.follow(caret);
                self.announce_caret();
            }
            SeatEvent::InputPanel { surface, shown, .. } => {
                tracing::debug!("{surface:?} on-screen keyboard requested: {shown}");
            }
            SeatEvent::InputPopup {
                seat,
                popup,
                parent,
                cursor,
            } => self.input_popup_shown(seat, popup, parent, cursor),
            SeatEvent::InputPopupGone { popup, .. } => self.input_popup_gone(popup),
            SeatEvent::Modifiers { held, .. } => {
                switcher_ctx!(self).switch_modifiers(modifiers(held))
            }
            _ => {}
        }
    }

    pub(super) fn cursor_shown(&mut self, image: CursorImage) {
        if !self.cursor.show(image) {
            return;
        }
        self.restrain(PointerRestrictions::HIDDEN, image == CursorImage::Hidden);
        self.announce_pointer_mode();
        self.backend.cursor_changed(&mut parts!(self));
        self.frames.request_all();
    }

    fn announce_pointer_mode(&mut self) {
        let hidden = self
            .input
            .restrictions
            .contains(PointerRestrictions::HIDDEN);
        let constraint = self
            .seats
            .get(self.input.seat)
            .and_then(|seat| seat.pointer().constraint());
        let mode = match constraint {
            Some((_, ConstraintKind::Lock)) => PointerMode::Relative,
            Some((_, ConstraintKind::Confine)) if hidden => PointerMode::Relative,
            Some((_, ConstraintKind::Confine)) | None => PointerMode::Absolute,
        };
        self.session.pointing.publish(mode);
    }

    fn restrain(&mut self, restraint: PointerRestrictions, on: bool) {
        if self.input.restrictions.contains(restraint) == on {
            return;
        }
        self.input.restrictions.set(restraint, on);
        self.backend.route_changed(&mut parts!(self));
    }
}
