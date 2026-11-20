use std::ops::Range;

use rikka_symbols::MaterialSymbol;
use rikka_types::Point;
use rikka_ui::{
    Accept, DRAG_SLOP, DragAction, DragActions, DragEnd, DragOffer, DragStart, Event, EventCtx,
    Offer,
};

use crate::dragging::drag_chip;
use crate::text::editing::Editing;
use crate::text::editing::state::EditingState;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DragOrigin {
    pub(crate) at: Point,
    pub(crate) offset: usize,
}

impl Editing {
    pub(crate) fn holds_selection_at(&self, offset: usize) -> bool {
        if self.options.secret {
            return false;
        }
        let selection = self.document.selection();
        !selection.is_empty() && selection.start <= offset && offset < selection.end
    }

    pub(crate) fn handle_drop_event(
        &self,
        state: &mut EditingState,
        event: &Event,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        match event {
            Event::PointerMoved(position) => match state.drag_origin {
                Some(drag_origin) if position.distance(drag_origin.at) > DRAG_SLOP => {
                    state.drag_origin = None;
                    ctx.release_pointer();
                    self.lift(state, ctx);
                    ctx.set_handled();
                    true
                }
                Some(_) => {
                    ctx.set_handled();
                    true
                }
                None => false,
            },
            Event::PointerReleased { .. } => {
                let Some(drag_origin) = state.drag_origin.take() else {
                    return false;
                };
                ctx.release_pointer();
                ctx.set_handled();
                if self.document.place_caret(drag_origin.offset, false) {
                    self.changed(ctx);
                }
                true
            }
            Event::DragMoved { position, offer } => self.hovered(state, *position, offer, ctx),
            Event::DragLeft => {
                if state.drop_at.take().is_some() {
                    ctx.request_paint();
                }
                false
            }
            Event::DragDropped {
                position,
                offer,
                action,
            } => self.accept_drop(state, *position, offer, *action, ctx),
            Event::DragEnded(end) => {
                self.finish_drag_out(state, *end, ctx);
                false
            }
            _ => false,
        }
    }

    fn lift(&self, state: &mut EditingState, ctx: &mut EventCtx<'_>) {
        let text = self.document.selected_text();
        if text.is_empty() {
            return;
        }
        let actions = match self.options.editable {
            true => DragActions::COPY_OR_MOVE,
            false => DragActions::COPY,
        };
        let icon = drag_chip(ctx.theme().clone(), MaterialSymbol::TEXT_FIELDS, &text);
        ctx.start_drag(DragStart::new(Offer::text(&text), icon).actions(actions));
        state.lifting = Some(self.document.selection());
        state.dropped_on_self = false;
    }

    fn hovered(
        &self,
        state: &mut EditingState,
        position: Point,
        offer: &DragOffer,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        let Some(mime) = self.takes(offer) else {
            return false;
        };
        let Some(offset) = self.offset_at(state, position) else {
            return false;
        };
        if state
            .lifting
            .as_ref()
            .is_some_and(|lifting| inside(lifting, offset))
        {
            if state.drop_at.take().is_some() {
                ctx.request_paint();
            }
            return false;
        }
        if state.drop_at != Some(offset) {
            state.drop_at = Some(offset);
            ctx.request_paint();
        }
        self.nudge_towards(state, position);
        ctx.accept_drag(Accept::copy_or_move(mime, offer));
        ctx.set_handled();
        true
    }

    fn accept_drop(
        &self,
        state: &mut EditingState,
        position: Point,
        offer: &DragOffer,
        action: DragAction,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        let at = state.drop_at.take();
        if self.takes(offer).is_none() {
            return false;
        }
        let Some(offset) = at.or_else(|| self.offset_at(state, position)) else {
            return false;
        };
        ctx.set_handled();
        ctx.request_focus();
        let moved = match (state.lifting.clone(), action) {
            (Some(lifting), DragAction::Move) => {
                state.dropped_on_self = true;
                self.shift(lifting, offset)
            }
            _ => match offer.text() {
                Some(text) => self.put(offset, &text),
                None => false,
            },
        };
        state.follow = true;
        if moved {
            self.changed(ctx);
        } else {
            ctx.request_paint();
        }
        true
    }

    fn finish_drag_out(&self, state: &mut EditingState, end: DragEnd, ctx: &mut EventCtx<'_>) {
        let Some(lifting) = state.lifting.take() else {
            return;
        };
        let dropped_on_self = std::mem::take(&mut state.dropped_on_self);
        if end != DragEnd::Landed(DragAction::Move) || dropped_on_self || !self.options.editable {
            return;
        }
        if self.document.replace(lifting, "") {
            self.changed(ctx);
        }
    }

    fn takes(&self, offer: &DragOffer) -> Option<String> {
        if !self.options.editable || self.options.secret {
            return None;
        }
        offer.text_mime().map(str::to_owned)
    }

    fn put(&self, offset: usize, text: &str) -> bool {
        let typed = self.acceptable(text);
        if typed.is_empty() {
            return false;
        }
        self.document.interrupt();
        if !self.document.replace(offset..offset, &typed) {
            return false;
        }
        self.document.select(offset, offset + typed.len());
        true
    }

    fn shift(&self, lifting: Range<usize>, to: usize) -> bool {
        if inside(&lifting, to) || to == lifting.start || to == lifting.end {
            return false;
        }
        let text = self.document.content().slice(lifting.clone());
        let length = text.len();
        let (edits, landed) = match to < lifting.start {
            true => (vec![(to..to, text), (lifting, String::new())], to),
            false => {
                let landed = to - length;
                (vec![(lifting, String::new()), (to..to, text)], landed)
            }
        };
        self.document.interrupt();
        if !self.document.replace_each(&edits) {
            return false;
        }
        self.document.select(landed, landed + length);
        true
    }
}

fn inside(range: &Range<usize>, offset: usize) -> bool {
    range.start < offset && offset < range.end
}
