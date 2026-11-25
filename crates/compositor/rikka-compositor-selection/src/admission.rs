use rikka_compositor_seat::SeatId;
use rikka_compositor_server::Client;
use rikka_wayland_wire::serial_not_before;

use crate::access::Access;
use crate::host::SelectionHost;
use crate::kind::SelectionKind;

pub(crate) fn live_seat<A: Access>(state: &mut A, seat: Option<SeatId>) -> Option<SeatId> {
    seat.filter(|seat| state.seats().get(*seat).is_some())
}

pub(crate) fn root<A: Access>(state: &mut A, seat: SeatId) -> SeatId {
    state.seats().carrier(seat).unwrap_or(seat)
}

pub(crate) fn selection_seat<A: Access>(state: &mut A, seat: Option<SeatId>) -> Option<SeatId> {
    let seat = live_seat(state, seat)?;
    Some(root(state, seat))
}

pub(crate) fn admits<S: SelectionHost>(
    state: &mut S,
    client: &Client,
    seat: SeatId,
    kind: SelectionKind,
    serial: u32,
) -> bool {
    if state.trusts_selection(client, seat, kind) {
        return true;
    }
    let issued = state.display().serial();
    if !serial_not_before(issued, serial) {
        return false;
    }
    let focused = state.seats().keyboard_clients(seat).contains(&client.id());
    let entry = state.selections().seat_mut(seat);
    let slot = entry.slot(kind);
    if slot.source.is_some() && !serial_not_before(serial, slot.serial) {
        return false;
    }
    focused
        && entry
            .focused_since(client.id())
            .is_none_or(|since| serial_not_before(serial, since))
}
