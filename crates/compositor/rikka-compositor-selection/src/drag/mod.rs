mod driven;
mod end;
mod feedback;
mod focus;
mod foreign;
mod grab;
mod icon;
mod negotiate;
mod offer;
mod session;
mod start;
mod toplevel;

use crate::access::Access;
use crate::ids::SourceId;
use crate::source::SourceRecord;

pub use driven::{
    cancel_compositor_drag, drop_compositor_drag, move_compositor_drag, start_compositor_drag,
};
pub(crate) use end::source_gone;
pub use foreign::{foreign_drag_data, foreign_drag_status, foreign_drop_finished};
pub(crate) use offer::{DragOffer, offer_destroyed, offer_request};
pub(crate) use session::Drag;
pub use session::{DragDevice, DragView};
pub(crate) use start::{DragRequest, refuse, start};

pub(crate) fn retire<A: Access>(state: &mut A, source: SourceId) {
    let selections = state.selections();
    let dragged = selections
        .sources
        .get(source)
        .is_some_and(SourceRecord::is_dragged);
    let dragging = selections
        .drags
        .values()
        .any(|drag| drag.source == Some(source));
    let offered = selections
        .drag_offers
        .values()
        .any(|offer| offer.source == source);
    if dragged && !dragging && !offered {
        selections.sources.remove(source);
    }
}
