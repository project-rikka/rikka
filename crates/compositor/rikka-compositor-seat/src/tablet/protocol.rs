use rikka_compositor_geometry::Point;
use rikka_compositor_protocols::zwp_tablet_manager_v2::{self as manager, ZwpTabletManagerV2};
use rikka_compositor_protocols::zwp_tablet_pad_v2::ZwpTabletPadV2;
use rikka_compositor_protocols::zwp_tablet_seat_v2::{self as tablet_seat, ZwpTabletSeatV2};
use rikka_compositor_protocols::zwp_tablet_tool_v2::{self as tool_protocol, ZwpTabletToolV2};
use rikka_compositor_protocols::zwp_tablet_v2::{self as tablet_protocol, ZwpTabletV2};
use rikka_wayland_wire::Resource;

use crate::access::Access;
use crate::host::SeatHost;
use crate::ids::{SeatId, TabletId, ToolId};
use crate::pointer::{CursorOwner, may_set_cursor, set_cursor};
use crate::tablet::announce::announce_all;

pub(crate) fn tablet_manager<A: Access>(
    state: &mut A,
    _manager: ZwpTabletManagerV2,
    request: manager::Request,
) {
    let manager::Request::GetTabletSeat { tablet_seat, seat } = request else {
        return;
    };
    let Some(seat) = seat.key::<SeatId>() else {
        return;
    };
    tablet_seat.set_key(seat);
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    entry.tablets.seats.push(tablet_seat.clone());
    announce_all(&mut entry.tablets, seat, &tablet_seat);
}

pub(crate) fn tablet_seat_request<A: Access>(
    _state: &mut A,
    _tablet_seat: ZwpTabletSeatV2,
    _request: tablet_seat::Request,
) {
}

pub(crate) fn tablet_seat_destroyed<A: Access>(state: &mut A, tablet_seat: ZwpTabletSeatV2) {
    let Some(seat) = tablet_seat.key::<SeatId>() else {
        return;
    };
    if let Some(entry) = state.seats().seat_mut(seat) {
        entry
            .tablets
            .seats
            .retain(|bound_seat| *bound_seat != tablet_seat);
    }
}

pub(crate) fn tablet_request<A: Access>(
    _state: &mut A,
    _tablet: ZwpTabletV2,
    _request: tablet_protocol::Request,
) {
}

pub(crate) fn tablet_destroyed<A: Access>(state: &mut A, tablet: ZwpTabletV2) {
    let (Some(seat), Some(id)) = (tablet.key::<SeatId>(), tablet.key::<TabletId>()) else {
        return;
    };
    if let Some(record) = state
        .seats()
        .seat_mut(seat)
        .and_then(|entry| entry.tablets.tablets.get_mut(id))
    {
        record.resources.retain(|resource| *resource != tablet);
    }
}

pub(crate) fn tool_request<S: SeatHost>(
    state: &mut S,
    tool: ZwpTabletToolV2,
    request: tool_protocol::Request,
) {
    let tool_protocol::Request::SetCursor {
        serial,
        surface,
        hotspot_x,
        hotspot_y,
    } = request
    else {
        return;
    };
    let (Some(seat), Some(id)) = (tool.key::<SeatId>(), tool.key::<ToolId>()) else {
        return;
    };
    let owner = CursorOwner::Tool(id);
    if !may_set_cursor(state, seat, owner, tool.client(), serial) {
        return;
    }
    if let Err(error) = set_cursor(
        state,
        seat,
        owner,
        surface,
        Point::new(hotspot_x, hotspot_y),
    ) {
        error.post(&tool, tool_protocol::Error::Role.code());
    }
}

pub(crate) fn tool_destroyed<A: Access>(state: &mut A, tool: ZwpTabletToolV2) {
    let (Some(seat), Some(id)) = (tool.key::<SeatId>(), tool.key::<ToolId>()) else {
        return;
    };
    if let Some(record) = state
        .seats()
        .seat_mut(seat)
        .and_then(|entry| entry.tablets.tools.get_mut(id))
    {
        record.resources.remove(&tool);
    }
}

pub(crate) fn pad_part_request<S: SeatHost, R: Resource>(
    _state: &mut S,
    _part: R,
    _request: R::Request<'_>,
) {
}

pub(crate) fn pad_part_destroyed<S: SeatHost, R: Resource>(state: &mut S, part: R) {
    let Some(pad) = ZwpTabletPadV2::from_object(part.object().clone()) else {
        return;
    };
    let (Some(seat), Some(id)) = (pad.key::<SeatId>(), pad.key::<crate::ids::PadId>()) else {
        return;
    };
    if let Some(record) = state
        .seats()
        .seat_mut(seat)
        .and_then(|entry| entry.tablets.pads.get_mut(id))
    {
        record.objects.retain(|objects| objects.pad != pad);
    }
}
