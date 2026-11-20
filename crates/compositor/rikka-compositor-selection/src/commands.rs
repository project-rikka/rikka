use std::os::fd::OwnedFd;

use rikka_compositor_seat::{SeatEvent, SeatId};

use crate::access::Access;
use crate::admission::{keyboard_client, live_seat};
use crate::error::{Error, Result};
use crate::ids::CompositorSource;
use crate::kind::SelectionKind;
use crate::mime::MimeList;
use crate::publish;
use crate::source::{self, Origin, SourceRecord};

pub fn set_compositor_selection<A: Access>(
    state: &mut A,
    seat: SeatId,
    kind: SelectionKind,
    mime_types: MimeList,
) -> Result<CompositorSource> {
    let seat = live_seat(state, Some(seat)).ok_or(Error::UnknownSeat)?;
    let record = SourceRecord::frozen(Origin::Compositor { seat, kind }, mime_types);
    let id = state.selections().sources.insert(record);
    let serial = state.display().serial();
    publish::replace(state, seat, kind, Some(id), serial);
    Ok(CompositorSource(id))
}

pub fn clear_selection<A: Access>(state: &mut A, seat: SeatId, kind: SelectionKind) {
    let Some(seat) = live_seat(state, Some(seat)) else {
        return;
    };
    let serial = state.display().serial();
    publish::replace(state, seat, kind, None, serial);
}

pub fn request_data<A: Access>(
    state: &mut A,
    seat: SeatId,
    kind: SelectionKind,
    mime_type: String,
    fd: OwnedFd,
) -> Result<()> {
    let seat = live_seat(state, Some(seat)).ok_or(Error::UnknownSeat)?;
    let selections = state.selections();
    let source = selections
        .seat(seat)
        .and_then(|entry| entry.slot(kind).source)
        .filter(|source| selections.sources.contains_key(*source))
        .ok_or(Error::NoSelection { kind })?;
    if !selections.sources[source].offers(&mime_type) {
        return Err(Error::Unoffered { kind, mime_type });
    }
    source::deliver(state, source, &mime_type, fd);
    Ok(())
}

pub fn request_drag_data<A: Access>(
    state: &mut A,
    seat: SeatId,
    mime_type: &str,
    fd: OwnedFd,
) -> Result<()> {
    let seat = live_seat(state, Some(seat)).ok_or(Error::UnknownSeat)?;
    let selections = state.selections();
    let source = selections
        .seat(seat)
        .and_then(|entry| entry.drag.as_ref())
        .and_then(|drag| drag.source)
        .filter(|source| selections.sources.contains_key(*source))
        .ok_or(Error::NoDrag)?;
    source::deliver(state, source, mime_type, fd);
    Ok(())
}

pub fn seat_event<A: Access>(state: &mut A, event: &SeatEvent) {
    if let SeatEvent::KeyboardFocus { seat, .. } = *event
        && let Some(seat) = live_seat(state, Some(seat))
    {
        focus_changed(state, seat);
    }
}

fn focus_changed<A: Access>(state: &mut A, seat: SeatId) {
    let client = keyboard_client(state, seat);
    let serial = state.display().serial();
    let entry = state.selections().seat_mut(seat);
    if entry.focus == client {
        return;
    }
    entry.focus = client;
    entry.focus_serial = serial;
    let Some(client) = client else {
        return;
    };
    for kind in SelectionKind::ALL {
        publish::announce_to(state, seat, kind, client);
    }
}
