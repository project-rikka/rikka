use rikka_compositor_protocols::wl_surface::WlSurface;
use rikka_compositor_seat::SeatId;
use rikka_compositor_surface::{Committed, Role, RoleError, SurfaceId, assign_role};

use crate::access::Access;
use crate::drag::session::Drag;
use crate::event::SelectionEvent;
use crate::host::SelectionHost;

#[derive(Debug)]
pub(crate) struct DragIcon;

impl<S: SelectionHost> Role<S> for DragIcon {
    const NAME: &'static str = "drag icon";

    fn committed(state: &mut S, commit: &Committed) {
        let Some((seat, drag)) = dragging(state, commit.surface) else {
            return;
        };
        drag.icon_offset += commit.offset;
        state.report(SelectionEvent::DragIconChanged { seat });
    }

    fn destroyed(state: &mut S, surface: SurfaceId, _role: Self) {
        let Some((seat, drag)) = dragging(state, surface) else {
            return;
        };
        drag.icon = None;
        state.report(SelectionEvent::DragIconChanged { seat });
    }
}

pub(crate) fn assign<S: SelectionHost>(
    state: &mut S,
    icon: &WlSurface,
) -> Result<SurfaceId, RoleError> {
    let surface = state.surfaces().lookup(icon).ok_or(RoleError::Gone)?;
    if dragging(state, surface).is_some() {
        return Err(RoleError::Active { role: "drag icon" });
    }
    if state.surfaces().role::<DragIcon>(surface).is_none() {
        assign_role(state, surface, DragIcon)?;
    }
    Ok(surface)
}

fn dragging<A: Access>(state: &mut A, surface: SurfaceId) -> Option<(SeatId, &mut Drag)> {
    state
        .selections()
        .drags
        .iter_mut()
        .find(|(_, drag)| drag.icon == Some(surface))
}
