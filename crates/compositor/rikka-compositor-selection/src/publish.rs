use rikka_compositor_seat::SeatId;
use rikka_wayland_wire::ClientId;

use crate::access::Access;
use crate::event::SelectionEvent;
use crate::ids::SourceId;
use crate::kind::SelectionKind;
use crate::offers::{self, Snapshot};
use crate::source::Origin;

pub(crate) fn replace<A: Access>(
    state: &mut A,
    seat: SeatId,
    kind: SelectionKind,
    source: Option<SourceId>,
    serial: u32,
) {
    let slot = state.selections().seat_mut(seat).slot_mut(kind);
    let previous = slot.source;
    slot.source = source;
    slot.serial = serial;
    if previous.is_none() && source.is_none() {
        return;
    }
    if let Some(previous) = previous.filter(|previous| Some(*previous) != source) {
        retire(state, previous);
    }
    announce(state, seat, kind);
    state.report(SelectionEvent::Changed { seat, kind });
}

pub(crate) fn renew<A: Access>(
    state: &mut A,
    seat: SeatId,
    kind: SelectionKind,
    source: SourceId,
) -> bool {
    if state.selections().seat_mut(seat).slot(kind).source != Some(source) {
        return false;
    }
    announce(state, seat, kind);
    state.report(SelectionEvent::Changed { seat, kind });
    true
}

pub(crate) fn source_gone<A: Access>(state: &mut A, source: SourceId) {
    let seats: Vec<SeatId> = state.selections().seats.keys().collect();
    for seat in seats {
        for kind in SelectionKind::ALL {
            let current = state.selections().seat_mut(seat).slot(kind).source;
            if current == Some(source) {
                let serial = state.display().serial();
                replace(state, seat, kind, None, serial);
            }
        }
    }
    state.selections().sources.remove(source);
}

pub(crate) fn announce<A: Access>(state: &mut A, seat: SeatId, kind: SelectionKind) {
    let focused: Vec<ClientId> = state
        .selections()
        .seat_mut(seat)
        .focus
        .iter()
        .map(|focused| focused.client)
        .collect();
    for client in focused {
        announce_to(state, seat, kind, client);
    }
    let selection = snapshot(state, seat, kind);
    let entry = state.selections().seat_mut(seat);
    for device in entry.control_devices.iter().filter(|device| device.alive()) {
        offers::to_control_device(device, kind, selection.as_ref());
    }
}

pub(crate) fn announce_to<A: Access>(
    state: &mut A,
    seat: SeatId,
    kind: SelectionKind,
    client: ClientId,
) {
    let selection = snapshot(state, seat, kind);
    let entry = state.selections().seat_mut(seat);
    match kind {
        SelectionKind::Clipboard => {
            for device in entry
                .data_devices
                .for_client(client)
                .iter()
                .filter(|device| device.alive())
            {
                offers::to_data_device(device, selection.as_ref());
            }
        }
        SelectionKind::Primary => {
            for device in entry
                .primary_devices
                .for_client(client)
                .iter()
                .filter(|device| device.alive())
            {
                offers::to_primary_device(device, selection.as_ref());
            }
        }
    }
}

pub(crate) fn snapshot<A: Access>(
    state: &mut A,
    seat: SeatId,
    kind: SelectionKind,
) -> Option<Snapshot> {
    let selections = state.selections();
    let source = selections.seat(seat)?.slot(kind).source?;
    let record = selections.sources.get(source)?;
    Some(Snapshot {
        source,
        mime_types: record.mime_types.frozen()?.clone(),
    })
}

pub(crate) fn retire<A: Access>(state: &mut A, source: SourceId) {
    let selections = state.selections();
    let Some(record) = selections.sources.get(source) else {
        return;
    };
    record.replaced();
    if matches!(record.origin, Origin::Compositor { .. }) {
        selections.sources.remove(source);
    }
}
