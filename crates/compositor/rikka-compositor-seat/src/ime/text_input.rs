use rikka_compositor_geometry::{Logical, Point, Rect, Size};
use rikka_compositor_protocols::zwp_text_input_manager_v3::{
    self as manager, ZwpTextInputManagerV3,
};
use rikka_compositor_protocols::zwp_text_input_v3::{
    self as protocol, ChangeCause, ContentHint, ContentPurpose, ZwpTextInputV3,
};

use crate::access::Access;
use crate::event::SeatEvent;
use crate::ids::SeatId;
use crate::ime::composing::{self, Surrounding};
use crate::surface;

#[derive(Debug, Default)]
struct Pending {
    enable: Option<bool>,
    surrounding: Option<(String, i32, i32)>,
    cause: Option<ChangeCause>,
    content: Option<(ContentHint, ContentPurpose)>,
    rect: Option<Rect<i32, Logical>>,
}

#[derive(Debug)]
pub(crate) struct TextInput {
    pub(crate) resource: ZwpTextInputV3,
    pub(crate) rect: Option<Rect<i32, Logical>>,
    commits: u32,
    pending: Pending,
}

impl TextInput {
    pub(crate) fn commits(&self) -> u32 {
        self.commits
    }
}

pub(crate) fn text_input_manager<A: Access>(
    state: &mut A,
    _manager: ZwpTextInputManagerV3,
    request: manager::Request,
) {
    let manager::Request::GetTextInput { id, seat } = request else {
        return;
    };
    let Some(seat) = seat.key::<SeatId>() else {
        return;
    };
    id.set_key(seat);
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    entry.ime.text_inputs.push(TextInput {
        resource: id.clone(),
        rect: None,
        commits: 0,
        pending: Pending::default(),
    });
    let focus = entry.ime.focus.filter(|_| entry.ime.entered);
    if let Some(wl) = focus.and_then(|focus| surface::resource(state, focus))
        && wl.client() == id.client()
    {
        id.enter(&wl);
    }
}

pub(crate) fn text_input_request<A: Access>(
    state: &mut A,
    text_input: ZwpTextInputV3,
    request: protocol::Request,
) {
    let Some(seat) = text_input.key::<SeatId>() else {
        return;
    };
    match request {
        protocol::Request::Destroy => {}
        protocol::Request::Enable => stage(state, seat, &text_input, |pending| {
            *pending = Pending {
                enable: Some(true),
                ..Pending::default()
            };
        }),
        protocol::Request::Disable => stage(state, seat, &text_input, |pending| {
            pending.enable = Some(false);
        }),
        protocol::Request::SetSurroundingText {
            text,
            cursor,
            anchor,
        } => stage(state, seat, &text_input, |pending| {
            pending.surrounding = Some((text.to_owned(), cursor, anchor));
        }),
        protocol::Request::SetTextChangeCause { cause } => {
            if let Ok(cause) = cause {
                stage(state, seat, &text_input, |pending| {
                    pending.cause = Some(cause)
                });
            }
        }
        protocol::Request::SetContentType { hint, purpose } => {
            if let Ok(purpose) = purpose {
                let hint = hint & known_hints(text_input.version());
                stage(state, seat, &text_input, |pending| {
                    pending.content = Some((hint, purpose));
                });
            }
        }
        protocol::Request::SetCursorRectangle {
            x,
            y,
            width,
            height,
        } => stage(state, seat, &text_input, |pending| {
            pending.rect = Some(Rect::new(Point::new(x, y), Size::new(width, height)));
        }),
        protocol::Request::SetAvailableActions { available_actions } => {
            if !valid_actions(available_actions) {
                text_input.post_error(protocol::Error::InvalidAction);
            }
        }
        protocol::Request::ShowInputPanel => panel(state, seat, &text_input, true),
        protocol::Request::HideInputPanel => panel(state, seat, &text_input, false),
        protocol::Request::Commit => commit(state, seat, &text_input),
    }
}

pub(crate) fn text_input_destroyed<A: Access>(state: &mut A, text_input: ZwpTextInputV3) {
    let Some(seat) = text_input.key::<SeatId>() else {
        return;
    };
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    entry
        .ime
        .text_inputs
        .retain(|input| input.resource != text_input);
    if entry.ime.composing.is(&text_input) {
        composing::end(state, seat);
    }
}

fn stage<A: Access>(
    state: &mut A,
    seat: SeatId,
    text_input: &ZwpTextInputV3,
    visit: impl FnOnce(&mut Pending),
) {
    if let Some(record) = state.seats().seat_mut(seat).and_then(|entry| {
        entry
            .ime
            .text_inputs
            .iter_mut()
            .find(|input| input.resource == *text_input)
    }) {
        visit(&mut record.pending);
    }
}

fn known_hints(version: u32) -> ContentHint {
    if version >= 2 {
        ContentHint::all()
    } else {
        ContentHint::all()
            - (ContentHint::ON_SCREEN_INPUT_PROVIDED
                | ContentHint::NO_EMOJI
                | ContentHint::PREEDIT_SHOWN)
    }
}

fn valid_actions(bytes: &[u8]) -> bool {
    let (chunks, rest) = bytes.as_chunks::<4>();
    if !rest.is_empty() {
        return false;
    }
    let actions: Vec<u32> = chunks
        .iter()
        .map(|chunk| u32::from_ne_bytes(*chunk))
        .collect();
    let unique = actions
        .iter()
        .enumerate()
        .all(|(index, action)| !actions[..index].contains(action));
    unique && !actions.contains(&0)
}

fn focused_surface<A: Access>(
    state: &mut A,
    seat: SeatId,
    text_input: &ZwpTextInputV3,
) -> Option<rikka_compositor_surface::SurfaceId> {
    let focus = state.seats().get(seat)?.ime.focus?;
    let wl = surface::resource(state, focus)?;
    (wl.client() == text_input.client()).then_some(focus)
}

fn panel<A: Access>(state: &mut A, seat: SeatId, text_input: &ZwpTextInputV3, shown: bool) {
    if let Some(surface) = focused_surface(state, seat, text_input) {
        state.report(SeatEvent::InputPanel {
            seat,
            surface,
            shown,
        });
    }
}

fn commit<A: Access>(state: &mut A, seat: SeatId, text_input: &ZwpTextInputV3) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let Some(record) = entry
        .ime
        .text_inputs
        .iter_mut()
        .find(|input| input.resource == *text_input)
    else {
        return;
    };
    record.commits = record.commits.wrapping_add(1);
    let pending = std::mem::take(&mut record.pending);
    let Some(surface) = focused_surface(state, seat, text_input) else {
        return;
    };
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let composing = entry.ime.composing.text_input().cloned();
    match pending.enable {
        Some(true) => {
            if composing
                .as_ref()
                .is_some_and(|active| active != text_input)
            {
                return;
            }
            if let Some(record) = entry
                .ime
                .text_inputs
                .iter_mut()
                .find(|input| input.resource == *text_input)
            {
                record.rect = None;
            }
            composing::begin(state, seat, text_input.clone(), surface);
        }
        Some(false) => {
            if composing.as_ref() == Some(text_input) {
                composing::end(state, seat);
            }
            return;
        }
        None if composing.as_ref() != Some(text_input) => return,
        None => {}
    }
    apply(state, seat, text_input, pending);
}

fn apply<A: Access>(state: &mut A, seat: SeatId, text_input: &ZwpTextInputV3, pending: Pending) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    if let Some(rect) = pending.rect
        && let Some(record) = entry
            .ime
            .text_inputs
            .iter_mut()
            .find(|input| input.resource == *text_input)
    {
        record.rect = Some(rect);
    }
    if let Some(method) = &entry.ime.method {
        let resource = &method.resource;
        if let Some((text, cursor, anchor)) = &pending.surrounding {
            resource.surrounding_text(text, *cursor as u32, *anchor as u32);
        }
        if let Some(cause) = pending.cause {
            resource.text_change_cause(cause);
        }
        if let Some((hint, purpose)) = pending.content {
            resource.content_type(hint, purpose);
        }
        resource.done();
    }
    if let Some(sent) = entry.ime.composing.sent.as_mut() {
        let mut changed = false;
        if let Some((text, cursor, anchor)) = pending.surrounding {
            sent.surrounding = Some(Surrounding {
                text,
                cursor: cursor.max(0) as u32,
                anchor: anchor.max(0) as u32,
            });
            changed = true;
        }
        if let Some((hint, purpose)) = pending.content {
            sent.hint = hint;
            sent.purpose = purpose;
            changed = true;
        }
        if changed || pending.enable == Some(true) {
            let sent = sent.clone();
            state.report(SeatEvent::TextInput {
                seat,
                state: Some(sent),
            });
        }
    }
    composing::announce(state, seat);
}
