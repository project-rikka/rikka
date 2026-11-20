use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_seat::SeatId;

use crate::access::Access;
use crate::drag::session::{drag_mut, drag_source};
use crate::event::SelectionEvent;

pub(crate) fn follow<A: Access>(state: &mut A, seat: SeatId, location: Point<f64, Logical>) {
    if !drag_mut(state, seat).is_some_and(|drag| drag.tethered) {
        return;
    }
    let Some(source) = drag_source(state, seat) else {
        return;
    };
    let attached = state
        .shell()
        .toplevel_drag(&source)
        .and_then(|drag| Some((drag.toplevel?, drag.offset)));
    let Some(drag) = drag_mut(state, seat) else {
        return;
    };
    let previous = drag.toplevel;
    drag.toplevel = attached.map(|(toplevel, _)| toplevel);
    if let Some(previous) = previous.filter(|previous| drag.toplevel != Some(*previous)) {
        state.report(SelectionEvent::DragToplevelReleased {
            seat,
            toplevel: previous,
        });
    }
    if let Some((toplevel, offset)) = attached {
        state.report(SelectionEvent::DragToplevelMoved {
            seat,
            toplevel,
            at: location - offset.to_f64(),
        });
    }
}

pub(crate) fn release<A: Access>(state: &mut A, seat: SeatId) {
    if let Some(toplevel) = drag_mut(state, seat).and_then(|drag| drag.toplevel.take()) {
        state.report(SelectionEvent::DragToplevelReleased { seat, toplevel });
    }
}
