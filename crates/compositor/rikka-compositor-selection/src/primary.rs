use rikka_compositor_protocols::zwp_primary_selection_device_manager_v1::{
    self as manager, ZwpPrimarySelectionDeviceManagerV1,
};
use rikka_compositor_protocols::zwp_primary_selection_device_v1::{
    self as device, ZwpPrimarySelectionDeviceV1,
};
use rikka_compositor_protocols::zwp_primary_selection_offer_v1::{
    self as offer, ZwpPrimarySelectionOfferV1,
};
use rikka_compositor_protocols::zwp_primary_selection_source_v1::{
    self as source, ZwpPrimarySelectionSourceV1,
};
use rikka_compositor_seat::SeatId;
use rikka_compositor_server::Client;

use crate::access::Access;
use crate::admission::{admits, live_seat, selection_seat};
use crate::host::SelectionHost;
use crate::ids::SourceId;
use crate::kind::SelectionKind;
use crate::offers;
use crate::publish::{self, snapshot};
use crate::source::{self as sources, Origin, SourceRecord};

pub(crate) fn manager_request<A: Access>(
    state: &mut A,
    _manager: ZwpPrimarySelectionDeviceManagerV1,
    request: manager::Request,
) {
    match request {
        manager::Request::CreateSource { id } => {
            let source = state
                .selections()
                .sources
                .insert(SourceRecord::new(Origin::Primary(id.clone())));
            id.set_key(source);
        }
        manager::Request::GetDevice { id, seat } => {
            let Some(seat) = selection_seat(state, seat.key::<SeatId>()) else {
                return;
            };
            id.set_key(seat);
            let entry = state.selections().seat_mut(seat);
            entry.primary_devices.add(id.clone());
            if entry.focused_since(id.client()).is_some() {
                let selection = snapshot(state, seat, SelectionKind::Primary);
                offers::to_primary_device(&id, selection.as_ref());
            }
        }
        manager::Request::Destroy => {}
    }
}

pub(crate) fn device_request<S: SelectionHost>(
    state: &mut S,
    device: ZwpPrimarySelectionDeviceV1,
    request: device::Request,
) {
    let device::Request::SetSelection { source, serial } = request else {
        return;
    };
    let Some(seat) = live_seat(state, device.key::<SeatId>()) else {
        return;
    };
    let Some(client) = Client::from_resource(&device) else {
        return;
    };
    let id = match &source {
        Some(source) => {
            let Some(id) = source.key::<SourceId>() else {
                return;
            };
            let Some(record) = state.selections().sources.get_mut(id) else {
                return;
            };
            if !record.is_fresh() {
                publish::renew(state, seat, SelectionKind::Primary, id);
                return;
            }
            record.freeze();
            Some(id)
        }
        None => None,
    };
    if !admits(state, &client, seat, SelectionKind::Primary, serial) {
        if let Some(source) = source {
            source.cancelled();
        }
        return;
    }
    publish::replace(state, seat, SelectionKind::Primary, id, serial);
}

pub(crate) fn device_destroyed<A: Access>(state: &mut A, device: ZwpPrimarySelectionDeviceV1) {
    let Some(seat) = device.key::<SeatId>() else {
        return;
    };
    if let Some(entry) = state.selections().seats.get_mut(seat) {
        entry.primary_devices.remove(&device);
    }
}

pub(crate) fn source_request<A: Access>(
    state: &mut A,
    source: ZwpPrimarySelectionSourceV1,
    request: source::Request,
) {
    if let source::Request::Offer { mime_type } = request
        && let Some(id) = source.key::<SourceId>()
        && let Some(record) = state.selections().sources.get_mut(id)
    {
        record.offer(mime_type);
    }
}

pub(crate) fn source_destroyed<A: Access>(state: &mut A, source: ZwpPrimarySelectionSourceV1) {
    if let Some(id) = source.key::<SourceId>() {
        publish::source_gone(state, id);
    }
}

pub(crate) fn offer_request<A: Access>(
    state: &mut A,
    offer: ZwpPrimarySelectionOfferV1,
    request: offer::Request,
) {
    if let offer::Request::Receive { mime_type, fd } = request
        && let Some(source) = offer.key::<SourceId>()
    {
        sources::deliver(state, source, mime_type, fd);
    }
}
