use rikka_compositor_protocols::wl_surface::WlSurface;
use rikka_compositor_protocols::zwp_text_input_v3::ZwpTextInputV3;
use rikka_compositor_surface::SurfaceId;

use crate::access::Access;
use crate::ids::SeatId;
use crate::ime::composing;
use crate::surface;

pub(crate) fn focus_left<A: Access>(state: &mut A, seat: SeatId) {
    composing::end(state, seat);
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let focus = entry.ime.focus.take();
    let entered = std::mem::replace(&mut entry.ime.entered, false);
    if !entered {
        return;
    }
    if let Some(wl) = focus.and_then(|focus| surface::resource(state, focus)) {
        for text_input in text_inputs_of(state, seat, &wl) {
            text_input.leave(&wl);
        }
    }
}

pub(crate) fn focus_entered<A: Access>(state: &mut A, seat: SeatId, surface: SurfaceId) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    entry.ime.focus = Some(surface);
    enter_focus(state, seat);
}

pub(crate) fn enter_focus<A: Access>(state: &mut A, seat: SeatId) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    if entry.ime.entered || (entry.ime.method.is_none() && !entry.ime.host) {
        return;
    }
    let Some(focus) = entry.ime.focus else {
        return;
    };
    let Some(wl) = surface::resource(state, focus) else {
        return;
    };
    if let Some(entry) = state.seats().seat_mut(seat) {
        entry.ime.entered = true;
    }
    for text_input in text_inputs_of(state, seat, &wl) {
        text_input.enter(&wl);
    }
}

pub(crate) fn leave_focus<A: Access>(state: &mut A, seat: SeatId) {
    composing::end(state, seat);
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    if !std::mem::replace(&mut entry.ime.entered, false) {
        return;
    }
    let focus = entry.ime.focus;
    let Some(wl) = focus.and_then(|focus| surface::resource(state, focus)) else {
        return;
    };
    for text_input in text_inputs_of(state, seat, &wl) {
        text_input.leave(&wl);
    }
}

pub(crate) fn text_inputs_of<A: Access>(
    state: &mut A,
    seat: SeatId,
    surface: &WlSurface,
) -> Vec<ZwpTextInputV3> {
    state
        .seats()
        .get(seat)
        .map(|entry| {
            entry
                .ime
                .text_inputs
                .iter()
                .map(|input| input.resource.clone())
                .filter(|text_input| text_input.client() == surface.client() && text_input.alive())
                .collect()
        })
        .unwrap_or_default()
}
