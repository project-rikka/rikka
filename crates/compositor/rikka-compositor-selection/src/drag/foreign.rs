use std::os::fd::OwnedFd;

use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_protocols::wl_data_device_manager::DndAction;
use rikka_compositor_seat::SeatId;
use rikka_compositor_surface::SurfaceId;

use crate::access::Access;
use crate::drag::feedback::Feedback;
use crate::drag::negotiate::renegotiate;
use crate::drag::offer::{DragOffer, Stage, mark_accepted};
use crate::drag::retire;
use crate::drag::session::{Target, drag_mut};
use crate::error::{Error, Result};
use crate::event::{ForeignDragEvent, SelectionEvent};
use crate::ids::{DragOfferId, ForeignOffer, SourceId};
use crate::source;

const RESOLVED: DndAction = DndAction::COPY.union(DndAction::MOVE);

pub(crate) fn enter<A: Access>(
    state: &mut A,
    seat: SeatId,
    source: SourceId,
    surface: SurfaceId,
    at: Point<f64, Logical>,
) {
    let id = state.selections().drag_offers.insert(DragOffer {
        source,
        seat,
        objects: Vec::new(),
        legacy: false,
        actions: DndAction::empty(),
        preferred: DndAction::empty(),
        accepted: false,
        chosen: DndAction::empty(),
        ask_chosen: false,
        stage: Stage::Hovering,
    });
    let Some(drag) = drag_mut(state, seat) else {
        state.selections().drag_offers.remove(id);
        return;
    };
    drag.target = Some(Target {
        surface,
        devices: Vec::new(),
        offer: Some(id),
        foreign: true,
    });
    emit(state, seat, id, ForeignDragEvent::Entered { surface, at });
}

pub(crate) fn emit<A: Access>(
    state: &mut A,
    seat: SeatId,
    offer: DragOfferId,
    event: ForeignDragEvent,
) {
    state.report(SelectionEvent::ForeignDrag {
        seat,
        offer: ForeignOffer(offer),
        event,
    });
}

pub fn foreign_drag_status<A: Access>(
    state: &mut A,
    offer: ForeignOffer,
    accepted: bool,
    action: DndAction,
) {
    let selections = state.selections();
    let Some(record) = selections
        .drag_offers
        .get(offer.0)
        .filter(|record| record.stage == Stage::Hovering)
    else {
        return;
    };
    let source = record.source;
    let accepted = accepted
        && selections
            .sources
            .get(source)
            .is_some_and(|source| !source.mime_types.as_slice().is_empty());
    let record = &mut selections.drag_offers[offer.0];
    record.actions = action;
    record.preferred = action;
    let changed = mark_accepted(state, offer.0, accepted);
    if changed && let Some(feedback) = Feedback::for_source(state, source) {
        let mime_type = accepted
            .then(|| {
                state
                    .selections()
                    .sources
                    .get(source)?
                    .mime_types
                    .as_slice()
                    .first()
                    .cloned()
            })
            .flatten();
        feedback.target(state, mime_type.as_deref());
    }
    renegotiate(state, offer.0);
}

pub fn foreign_drop_finished<A: Access>(
    state: &mut A,
    offer: ForeignOffer,
    succeeded: bool,
    action: Option<DndAction>,
) {
    let Some(record) = state.selections().drag_offers.remove(offer.0) else {
        return;
    };
    if record.stage != Stage::Dropped {
        return;
    }
    if let Some(feedback) = Feedback::for_source(state, record.source) {
        let chosen = action.filter(|action| RESOLVED.contains(*action) && !action.is_empty());
        let resolved = chosen.unwrap_or(record.chosen);
        match succeeded && record.accepted && resolved.intersects(RESOLVED) {
            true => feedback.finished(state, record.ask_chosen.then_some(resolved)),
            false => feedback.cancelled(state),
        }
    }
    retire(state, record.source);
}

pub fn foreign_drag_data<A: Access>(
    state: &mut A,
    offer: ForeignOffer,
    mime_type: String,
    fd: OwnedFd,
) -> Result<()> {
    let record = state
        .selections()
        .drag_offers
        .get(offer.0)
        .filter(|record| matches!(record.stage, Stage::Hovering | Stage::Dropped))
        .ok_or(Error::NoDrag)?;
    let source = record.source;
    source::deliver(state, source, &mime_type, fd);
    Ok(())
}

pub(crate) fn dropped<A: Access>(state: &mut A, seat: SeatId, id: DragOfferId) -> bool {
    let Some(record) = state.selections().drag_offers.get_mut(id) else {
        return false;
    };
    if record.stage != Stage::Hovering || !record.accepted || record.chosen.is_empty() {
        return false;
    }
    record.stage = Stage::Dropped;
    record.ask_chosen = record.chosen == DndAction::ASK;
    emit(state, seat, id, ForeignDragEvent::Dropped);
    true
}
