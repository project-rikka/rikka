use rikka_compositor_protocols::zwp_text_input_v3::{ContentHint, ContentPurpose, ZwpTextInputV3};
use rikka_compositor_surface::SurfaceId;

use crate::access::Access;
use crate::event::{Caret, SeatEvent};
use crate::ids::SeatId;
use crate::ime::Composition;
use crate::ime::popup::announce_popups;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Surrounding {
    pub text: String,
    pub cursor: u32,
    pub anchor: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextInputState {
    pub surface: SurfaceId,
    pub surrounding: Option<Surrounding>,
    pub hint: ContentHint,
    pub purpose: ContentPurpose,
}

#[derive(Debug, Default)]
pub(crate) struct Composing {
    active: Option<(ZwpTextInputV3, SurfaceId)>,
    announced: Option<Caret>,
    pub(crate) sent: Option<TextInputState>,
}

impl Composing {
    pub(crate) fn surface(&self) -> Option<SurfaceId> {
        self.active.as_ref().map(|(_, surface)| *surface)
    }

    pub(crate) fn text_input(&self) -> Option<&ZwpTextInputV3> {
        self.active.as_ref().map(|(text_input, _)| text_input)
    }

    pub(crate) fn is(&self, text_input: &ZwpTextInputV3) -> bool {
        self.text_input() == Some(text_input)
    }
}

pub(crate) fn begin<A: Access>(
    state: &mut A,
    seat: SeatId,
    text_input: ZwpTextInputV3,
    surface: SurfaceId,
) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    entry.ime.composing.active = Some((text_input, surface));
    if let Some(method) = &entry.ime.method {
        method.resource.activate();
    }
    if entry.ime.host {
        entry.ime.composing.sent = Some(TextInputState {
            surface,
            surrounding: None,
            hint: ContentHint::empty(),
            purpose: ContentPurpose::Normal,
        });
    }
}

pub(crate) fn end<A: Access>(state: &mut A, seat: SeatId) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    if entry.ime.composing.active.take().is_none() {
        return;
    }
    if let Some(method) = &entry.ime.method {
        method.resource.deactivate();
        method.resource.done();
    }
    if entry.ime.composing.sent.take().is_some() {
        state.report(SeatEvent::TextInput { seat, state: None });
    }
    announce(state, seat);
}

pub(crate) fn deliver<A: Access>(state: &mut A, seat: SeatId, composition: &Composition) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let Some(text_input) = entry.ime.composing.text_input() else {
        return;
    };
    let Some(serial) = entry
        .ime
        .text_inputs
        .iter()
        .find(|input| input.resource == *text_input)
        .map(|input| input.commits())
    else {
        return;
    };
    if let Some((text, begin, end)) = &composition.preedit {
        text_input.preedit_string(Some(text.as_str()), *begin, *end);
    }
    if let Some(text) = &composition.commit {
        text_input.commit_string(Some(text.as_str()));
    }
    if let Some((before, after)) = composition.delete {
        text_input.delete_surrounding_text(before, after);
    }
    text_input.done(serial);
}

pub(crate) fn announce<A: Access>(state: &mut A, seat: SeatId) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let ime = &mut entry.ime;
    let caret = ime
        .composing
        .active
        .as_ref()
        .and_then(|(text_input, surface)| {
            let rect = ime
                .text_inputs
                .iter()
                .find(|input| input.resource == *text_input)?
                .rect?;
            Some(Caret {
                surface: *surface,
                rect,
            })
        });
    let changed = ime.composing.announced != caret;
    ime.composing.announced = caret;
    announce_popups(state, seat);
    if changed {
        state.report(SeatEvent::Caret { seat, caret });
    }
}
