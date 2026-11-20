use crate::access::Access;
use crate::ids::SeatId;
use crate::ime::Composition;
use crate::ime::composing;
use crate::ime::focus::{enter_focus, leave_focus};

pub(crate) fn serve_text_input<A: Access>(state: &mut A, seat: SeatId, served: bool) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    if entry.ime.host == served {
        return;
    }
    entry.ime.host = served;
    match served {
        true => enter_focus(state, seat),
        false
            if state
                .seats()
                .get(seat)
                .is_some_and(|entry| entry.ime.method.is_none()) =>
        {
            leave_focus(state, seat)
        }
        false => {}
    }
}

pub(crate) fn compose<A: Access>(state: &mut A, seat: SeatId, composition: &Composition) {
    let served = state
        .seats()
        .get(seat)
        .is_some_and(|entry| entry.ime.host && entry.ime.composing.sent.is_some());
    if served {
        composing::deliver(state, seat, composition);
    }
}
