use rikka_compositor_seat::SeatId;
use rikka_compositor_server::Client;
use rikka_wayland_wire::{ClientId, serial_not_before};

use crate::access::Access;
use crate::host::SelectionHost;
use crate::kind::SelectionKind;

pub(crate) fn live_seat<A: Access>(state: &mut A, seat: Option<SeatId>) -> Option<SeatId> {
    seat.filter(|seat| state.seats().get(*seat).is_some())
}

pub(crate) fn keyboard_client<A: Access>(state: &mut A, seat: SeatId) -> Option<ClientId> {
    let focus = state.seats().get(seat)?.keyboard().focus()?;
    state
        .surfaces()
        .get(focus.surface())
        .map(|surface| surface.client())
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
    let focused = keyboard_client(state, seat) == Some(client.id());
    let entry = state.selections().seat_mut(seat);
    let slot = entry.slot(kind);
    if slot.source.is_some() && !serial_not_before(serial, slot.serial) {
        return false;
    }
    focused && (entry.focus != Some(client.id()) || serial_not_before(serial, entry.focus_serial))
}
