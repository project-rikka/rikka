use std::time::Duration;

use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_protocols::wl_data_device_manager::DndAction;
use rikka_compositor_seat::{Focus, Motion, SeatId};

use crate::access::Access;
use crate::admission::live_seat;
use crate::drag::end::finish;
use crate::drag::focus::moved;
use crate::drag::session::{Drag, DragDevice, drag_mut};
use crate::error::{Error, Result};
use crate::event::SelectionEvent;
use crate::ids::CompositorSource;
use crate::mime::MimeList;
use crate::source::{Origin, SourceRecord};

pub fn start_compositor_drag<A: Access>(
    state: &mut A,
    seat: SeatId,
    mime_types: MimeList,
    actions: DndAction,
    at: Point<f64, Logical>,
) -> Result<CompositorSource> {
    let seat = live_seat(state, Some(seat)).ok_or(Error::UnknownSeat)?;
    if state.selections().drags.contains_key(seat) {
        return Err(Error::Dragging);
    }
    let mut record = SourceRecord::frozen(Origin::Dragged { seat }, mime_types);
    record.actions = Some(actions);
    let id = state.selections().sources.insert(record);
    let mut drag = Drag::new(Some(id), None, DragDevice::Pointer, at, None);
    drag.driven = true;
    state.selections().drags.insert(seat, drag);
    state.report(SelectionEvent::DragStarted { seat });
    Ok(CompositorSource(id))
}

pub fn move_compositor_drag<A: Access>(
    state: &mut A,
    seat: SeatId,
    location: Point<f64, Logical>,
    under: Option<Focus>,
    time: Duration,
) {
    if !driven(state, seat) {
        return;
    }
    moved(
        state,
        seat,
        &Motion {
            location,
            under,
            time,
        },
    );
}

pub fn drop_compositor_drag<A: Access>(state: &mut A, seat: SeatId) {
    let Some(drag) = drag_mut(state, seat).filter(|drag| drag.driven) else {
        return;
    };
    drag.released = true;
    finish(state, seat);
}

pub fn cancel_compositor_drag<A: Access>(state: &mut A, seat: SeatId) {
    if driven(state, seat) {
        finish(state, seat);
    }
}

fn driven<A: Access>(state: &mut A, seat: SeatId) -> bool {
    drag_mut(state, seat).is_some_and(|drag| drag.driven)
}
