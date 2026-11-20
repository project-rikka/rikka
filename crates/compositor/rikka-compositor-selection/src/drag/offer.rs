use rikka_compositor_protocols::wl_data_device::WlDataDevice;
use rikka_compositor_protocols::wl_data_device_manager::DndAction;
use rikka_compositor_protocols::wl_data_offer::{Error, Request, WlDataOffer};
use rikka_compositor_seat::SeatId;

use crate::access::Access;
use crate::data_device::OfferTarget;
use crate::drag::feedback::Feedback;
use crate::drag::negotiate::renegotiate;
use crate::drag::retire;
use crate::event::SelectionEvent;
use crate::ids::{DragOfferId, SourceId};
use crate::offers;
use crate::source::{self, ACTIONS_SINCE};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stage {
    Hovering,
    Dropped,
    Finished,
    Left,
}

#[derive(Debug)]
pub(crate) struct DragOffer {
    pub(crate) source: SourceId,
    pub(crate) seat: SeatId,
    pub(crate) objects: Vec<WlDataOffer>,
    pub(crate) legacy: bool,
    pub(crate) actions: DndAction,
    pub(crate) preferred: DndAction,
    pub(crate) accepted: bool,
    pub(crate) chosen: DndAction,
    pub(crate) ask_chosen: bool,
    pub(crate) stage: Stage,
}

pub(crate) fn create<A: Access>(
    state: &mut A,
    seat: SeatId,
    source: SourceId,
    devices: &[WlDataDevice],
) -> Option<(DragOfferId, Vec<(WlDataDevice, WlDataOffer)>)> {
    let selections = state.selections();
    let record = selections.sources.get(source)?;
    let source_actions = record.drag_actions();
    let pairs: Vec<(WlDataDevice, WlDataOffer)> = devices
        .iter()
        .filter_map(|device| Some((device.clone(), offers::create::<WlDataOffer>(device)?)))
        .collect();
    let objects: Vec<WlDataOffer> = pairs.iter().map(|(_, object)| object.clone()).collect();
    let id = selections.drag_offers.insert(DragOffer {
        source,
        seat,
        legacy: objects
            .iter()
            .all(|object| object.version() < ACTIONS_SINCE),
        objects,
        actions: DndAction::empty(),
        preferred: DndAction::empty(),
        accepted: false,
        chosen: DndAction::empty(),
        ask_chosen: false,
        stage: Stage::Hovering,
    });
    for (device, object) in &pairs {
        OfferTarget::Drag(id).attach(object);
        device.data_offer(object);
        for mime_type in record.mime_types.as_slice() {
            object.offer(mime_type);
        }
        object.source_actions(source_actions);
    }
    Some((id, pairs))
}

pub(crate) fn offer_request<A: Access>(
    state: &mut A,
    offer: &WlDataOffer,
    id: DragOfferId,
    request: Request,
) {
    let Some(record) = state.selections().drag_offers.get(id) else {
        if matches!(request, Request::Finish) {
            offer.post_error(Error::InvalidFinish);
        }
        return;
    };
    let stage = record.stage;
    if stage == Stage::Finished && !matches!(request, Request::Destroy) {
        return offer.post_error(Error::InvalidOffer);
    }
    let live = matches!(stage, Stage::Hovering | Stage::Dropped);
    match request {
        Request::Destroy => {}
        Request::Accept { mime_type, .. } if live => accept(state, id, mime_type),
        Request::Receive { mime_type, fd } if live => {
            let source = record.source;
            source::deliver(state, source, mime_type, fd);
        }
        Request::SetActions {
            dnd_actions,
            preferred_action,
        } => set_actions(state, offer, id, dnd_actions, preferred_action),
        Request::Finish => finish(state, offer, id),
        Request::Accept { .. } | Request::Receive { .. } => {}
    }
}

pub(crate) fn offer_destroyed<A: Access>(state: &mut A, offer: &WlDataOffer, id: DragOfferId) {
    let selections = state.selections();
    let Some(record) = selections.drag_offers.get_mut(id) else {
        return;
    };
    record.objects.retain(|object| object != offer);
    if !record.objects.is_empty() {
        return;
    }
    let Some(record) = selections.drag_offers.remove(id) else {
        return;
    };
    if record.stage == Stage::Dropped
        && let Some(feedback) = Feedback::for_source(state, record.source)
    {
        match offer.version() < ACTIONS_SINCE {
            true => feedback.finished(state, None),
            false => feedback.cancelled(state),
        }
    }
    retire(state, record.source);
}

fn accept<A: Access>(state: &mut A, id: DragOfferId, mime_type: Option<&str>) {
    let selections = state.selections();
    let record = &selections.drag_offers[id];
    let Some(source) = selections.sources.get(record.source) else {
        return;
    };
    let accepted = mime_type.is_some_and(|mime_type| source.offers(mime_type));
    let (hovering, source) = (record.stage == Stage::Hovering, record.source);
    mark_accepted(state, id, accepted);
    if hovering && let Some(feedback) = Feedback::for_source(state, source) {
        feedback.target(state, mime_type);
    }
}

pub(crate) fn mark_accepted<A: Access>(state: &mut A, id: DragOfferId, accepted: bool) -> bool {
    let record = &mut state.selections().drag_offers[id];
    if record.accepted == accepted {
        return false;
    }
    record.accepted = accepted;
    let (hovering, seat) = (record.stage == Stage::Hovering, record.seat);
    if hovering {
        state.report(SelectionEvent::DragAccepted { seat });
    }
    true
}

fn set_actions<A: Access>(
    state: &mut A,
    offer: &WlDataOffer,
    id: DragOfferId,
    actions: DndAction,
    preferred: DndAction,
) {
    if !DndAction::all().contains(actions) {
        return offer.post_error(Error::InvalidActionMask);
    }
    if !DndAction::all().contains(preferred) || preferred.bits().count_ones() > 1 {
        return offer.post_error(Error::InvalidAction);
    }
    let selections = state.selections();
    let record = &selections.drag_offers[id];
    let asking = record.stage == Stage::Dropped && record.ask_chosen;
    if record.stage != Stage::Hovering && !asking {
        return;
    }
    if asking {
        let source_actions = selections
            .sources
            .get(record.source)
            .map_or(DndAction::empty(), |source| source.drag_actions());
        if !source_actions.contains(preferred) {
            return offer.post_error(Error::InvalidAction);
        }
    }
    let record = &mut selections.drag_offers[id];
    record.actions = actions;
    record.preferred = preferred;
    renegotiate(state, id);
}

fn finish<A: Access>(state: &mut A, offer: &WlDataOffer, id: DragOfferId) {
    let selections = state.selections();
    let record = &mut selections.drag_offers[id];
    let resolved = record.chosen.intersects(DndAction::COPY | DndAction::MOVE);
    if record.stage != Stage::Dropped || !record.accepted || !resolved {
        return offer.post_error(Error::InvalidFinish);
    }
    record.stage = Stage::Finished;
    let action = record.ask_chosen.then_some(record.chosen);
    let source = record.source;
    if let Some(feedback) = Feedback::for_source(state, source) {
        feedback.finished(state, action);
    }
}
