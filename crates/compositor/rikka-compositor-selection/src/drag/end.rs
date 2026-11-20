use rikka_compositor_protocols::wl_data_device_manager::DndAction;
use rikka_compositor_protocols::wl_data_source::WlDataSource;
use rikka_compositor_seat::SeatId;

use crate::access::Access;
use crate::drag::feedback::Feedback;
use crate::drag::focus::leave_target;
use crate::drag::foreign;
use crate::drag::offer::Stage;
use crate::drag::retire;
use crate::drag::session::{drag_mut, drag_source, end_drag_grab};
use crate::drag::toplevel;
use crate::event::SelectionEvent;
use crate::ids::SourceId;
use crate::source::ACTIONS_SINCE;

pub(crate) fn finish<A: Access>(state: &mut A, seat: SeatId) {
    let Some((released, source, carries_window, tethered)) = drag_mut(state, seat).map(|drag| {
        (
            drag.released,
            drag.source,
            drag.toplevel.is_some(),
            drag.tethered,
        )
    }) else {
        return;
    };
    let feedback = source.and_then(|source| Feedback::for_source(state, source));
    let dropped = released && drop(state, seat);
    if !dropped && let Some(feedback) = &feedback {
        if released && carries_window {
            feedback.performed(state);
        }
        feedback.cancelled(state);
    }
    leave_target(state, seat);
    toplevel::release(state, seat);
    if tethered && let Some(source) = drag_source(state, seat) {
        state.shell().toplevel_drag_ended(&source);
    }
    if let Some(entry) = state.selections().seats.get_mut(seat) {
        entry.drag = None;
    }
    if let Some(source) = source {
        retire(state, source);
    }
    state.report(SelectionEvent::DragEnded { seat, dropped });
}

pub(crate) fn source_gone<A: Access>(state: &mut A, source: SourceId) {
    let seats: Vec<SeatId> = state
        .selections()
        .seats
        .iter()
        .filter(|(_, entry)| {
            entry
                .drag
                .as_ref()
                .is_some_and(|drag| drag.source == Some(source))
        })
        .map(|(seat, _)| seat)
        .collect();
    for seat in seats {
        end_drag_grab(state, seat);
    }
}

pub(crate) fn cancel_source(source: &WlDataSource) {
    if source.version() >= ACTIONS_SINCE {
        source.cancelled();
    }
}

fn drop<A: Access>(state: &mut A, seat: SeatId) -> bool {
    let Some(drag) = drag_mut(state, seat) else {
        return false;
    };
    let Some(target) = drag.target.clone() else {
        return false;
    };
    let source = drag.source;
    if target.foreign {
        let dropped = target
            .offer
            .is_some_and(|offer| foreign::dropped(state, seat, offer));
        if dropped
            && let Some(feedback) = source.and_then(|source| Feedback::for_source(state, source))
        {
            feedback.performed(state);
        }
        return dropped;
    }
    if target.devices.is_empty() {
        return false;
    }
    if source.is_some() {
        let Some(id) = target.offer else {
            return false;
        };
        let Some(offer) = state.selections().drag_offers.get_mut(id) else {
            return false;
        };
        if offer.stage != Stage::Hovering || !offer.accepted || offer.chosen.is_empty() {
            return false;
        }
        offer.stage = Stage::Dropped;
        offer.ask_chosen = offer.chosen == DndAction::ASK;
    }
    for device in target.devices.iter().filter(|device| device.alive()) {
        device.drop();
    }
    if let Some(feedback) = source.and_then(|source| Feedback::for_source(state, source)) {
        feedback.performed(state);
    }
    true
}
