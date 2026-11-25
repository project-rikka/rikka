use rikka_compositor_protocols::wl_data_device_manager::DndAction;

use crate::access::Access;
use crate::drag::feedback::Feedback;
use crate::drag::offer::Stage;
use crate::ids::DragOfferId;

pub(crate) fn choose(available: DndAction, preferred: DndAction) -> DndAction {
    if available.is_empty() {
        return DndAction::empty();
    }
    if available.intersects(preferred) {
        return preferred;
    }
    DndAction::from_bits_retain(1 << available.bits().trailing_zeros())
}

pub(crate) fn renegotiate<A: Access>(state: &mut A, id: DragOfferId) {
    let selections = state.selections();
    let Some(offer) = selections.drag_offers.get(id) else {
        return;
    };
    let Some(record) = selections.sources.get(offer.source) else {
        return;
    };
    let source_actions = record.drag_actions();
    let source = offer.source;
    let (offer_actions, preferred) = if offer.legacy {
        (DndAction::COPY, DndAction::empty())
    } else {
        (offer.actions, offer.preferred)
    };
    let chosen = choose(source_actions & offer_actions, preferred);
    let seat = offer.seat;
    let offer = &mut selections.drag_offers[id];
    match offer.stage {
        Stage::Hovering => {}
        Stage::Dropped if offer.ask_chosen => {
            offer.chosen = chosen;
            return;
        }
        Stage::Dropped | Stage::Finished | Stage::Left => return,
    }
    if offer.chosen != chosen {
        offer.chosen = chosen;
        for object in offer.objects.iter().filter(|object| object.alive()) {
            object.action(chosen);
        }
    }
    let Some(drag) = selections.drags.get_mut(seat) else {
        return;
    };
    if drag.action != chosen {
        drag.action = chosen;
        if let Some(feedback) = Feedback::for_source(state, source) {
            feedback.action(state, chosen);
        }
    }
}
