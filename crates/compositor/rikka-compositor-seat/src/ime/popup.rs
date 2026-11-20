use rikka_compositor_geometry::{Logical, Point, Rect};
use rikka_compositor_protocols::wl_surface::WlSurface;
use rikka_compositor_protocols::zwp_input_method_v2::ZwpInputMethodV2;
use rikka_compositor_protocols::zwp_input_popup_surface_v2::{
    self as protocol, ZwpInputPopupSurfaceV2,
};
use rikka_compositor_surface::{BufferChange, Changes, Committed, Role, SurfaceId, assign_role};

use crate::access::Access;
use crate::event::SeatEvent;
use crate::host::SeatHost;
use crate::ids::SeatId;
use crate::surface;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PopupPlacement {
    parent: Option<SurfaceId>,
    cursor: Option<Rect<i32, Logical>>,
}

#[derive(Debug)]
pub(crate) struct Popup {
    resource: ZwpInputPopupSurfaceV2,
    surface: SurfaceId,
    at: Option<Point<i32, Logical>>,
    sent: Option<Rect<i32, Logical>>,
    announced: Option<PopupPlacement>,
}

#[derive(Debug)]
struct InputPopupRole {
    seat: SeatId,
}

impl<S: SeatHost> Role<S> for InputPopupRole {
    const NAME: &'static str = "zwp_input_popup_surface_v2";

    fn committed(state: &mut S, commit: &Committed) {
        if commit.buffer == BufferChange::Unchanged && !commit.changes.contains(Changes::SIZE) {
            return;
        }
        let Some(seat) = state
            .surfaces()
            .role::<InputPopupRole>(commit.surface)
            .map(|role| role.seat)
        else {
            return;
        };
        if let Some(popup) = state.seats().seat_mut(seat).and_then(|entry| {
            entry
                .ime
                .popups
                .iter_mut()
                .find(|popup| popup.surface == commit.surface)
        }) {
            popup.announced = None;
        }
        announce_popups(state, seat);
    }
}

pub(crate) fn open_popup<S: SeatHost>(
    state: &mut S,
    seat: SeatId,
    input_method: &ZwpInputMethodV2,
    popup: ZwpInputPopupSurfaceV2,
    wl: WlSurface,
) {
    let Some(surface) = surface::id(state, &wl) else {
        return;
    };
    let reused = state
        .surfaces()
        .role_mut::<InputPopupRole>(surface)
        .map(|role| role.seat = seat)
        .is_some();
    if !reused && let Err(error) = assign_role(state, surface, InputPopupRole { seat }) {
        error.post(input_method, 0);
        return;
    }
    popup.set_key(seat);
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    entry.ime.popups.push(Popup {
        resource: popup,
        surface,
        at: None,
        sent: None,
        announced: None,
    });
    announce_popups(state, seat);
}

pub(crate) fn popup_request<A: Access>(
    _state: &mut A,
    _popup: ZwpInputPopupSurfaceV2,
    _request: protocol::Request,
) {
}

pub(crate) fn popup_destroyed<A: Access>(state: &mut A, popup: ZwpInputPopupSurfaceV2) {
    let Some(seat) = popup.key::<SeatId>() else {
        return;
    };
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let Some(index) = entry
        .ime
        .popups
        .iter()
        .position(|input_popup| input_popup.resource == popup)
    else {
        return;
    };
    let removed = entry.ime.popups.remove(index);
    state.surfaces().end_role::<InputPopupRole>(removed.surface);
    state.report(SeatEvent::InputPopupGone {
        seat,
        popup: removed.surface,
    });
}

pub(crate) fn popups_surface_gone<A: Access>(state: &mut A, seat: SeatId, surface: SurfaceId) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let before = entry.ime.popups.len();
    entry.ime.popups.retain(|popup| popup.surface != surface);
    if entry.ime.popups.len() != before {
        state.report(SeatEvent::InputPopupGone {
            seat,
            popup: surface,
        });
    }
}

pub(crate) fn place_popup<A: Access>(
    state: &mut A,
    seat: SeatId,
    popup: SurfaceId,
    at: Point<i32, Logical>,
) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let cursor = composing_cursor(&entry.ime);
    let Some(record) = entry
        .ime
        .popups
        .iter_mut()
        .find(|input_popup| input_popup.surface == popup)
    else {
        return;
    };
    record.at = Some(at);
    send_rectangle(record, cursor);
}

pub(crate) fn announce_popups<A: Access>(state: &mut A, seat: SeatId) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let now = PopupPlacement {
        parent: entry.ime.composing.surface(),
        cursor: composing_cursor(&entry.ime),
    };
    let mut events = Vec::new();
    for popup in &mut entry.ime.popups {
        send_rectangle(popup, now.cursor);
        if popup.announced == Some(now) {
            continue;
        }
        popup.announced = Some(now);
        events.push(SeatEvent::InputPopup {
            seat,
            popup: popup.surface,
            parent: now.parent,
            cursor: now.cursor,
        });
    }
    for event in events {
        state.report(event);
    }
}

fn composing_cursor(ime: &crate::ime::Ime) -> Option<Rect<i32, Logical>> {
    let text_input = ime.composing.text_input()?;
    ime.text_inputs
        .iter()
        .find(|input| input.resource == *text_input)?
        .rect
}

fn send_rectangle(popup: &mut Popup, cursor: Option<Rect<i32, Logical>>) {
    let (Some(at), Some(cursor)) = (popup.at, cursor) else {
        return;
    };
    let relative = Rect::new(cursor.loc - at, cursor.size);
    if popup.sent == Some(relative) {
        return;
    }
    popup.sent = Some(relative);
    popup.resource.text_input_rectangle(
        relative.loc.x,
        relative.loc.y,
        relative.size.w,
        relative.size.h,
    );
}
