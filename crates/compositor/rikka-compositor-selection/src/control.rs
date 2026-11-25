use rikka_compositor_protocols::ext_data_control_device_v1::{
    self as device, ExtDataControlDeviceV1,
};
use rikka_compositor_protocols::ext_data_control_manager_v1::{
    self as manager, ExtDataControlManagerV1,
};
use rikka_compositor_protocols::ext_data_control_offer_v1::{self as offer, ExtDataControlOfferV1};
use rikka_compositor_protocols::ext_data_control_source_v1::{
    self as source, ExtDataControlSourceV1,
};
use rikka_compositor_seat::SeatId;

use crate::access::Access;
use crate::admission::{live_seat, selection_seat};
use crate::ids::SourceId;
use crate::kind::SelectionKind;
use crate::offers;
use crate::publish::{self, snapshot};
use crate::source::{self as sources, Origin, SourceRecord};

pub(crate) fn manager_request<A: Access>(
    state: &mut A,
    _manager: ExtDataControlManagerV1,
    request: manager::Request,
) {
    match request {
        manager::Request::CreateDataSource { id } => {
            let source = state
                .selections()
                .sources
                .insert(SourceRecord::new(Origin::Control(id.clone())));
            id.set_key(source);
        }
        manager::Request::GetDataDevice { id, seat } => {
            let Some(seat) = selection_seat(state, seat.key::<SeatId>()) else {
                id.finished();
                return;
            };
            id.set_key(seat);
            state
                .selections()
                .seat_mut(seat)
                .control_devices
                .push(id.clone());
            for kind in SelectionKind::ALL {
                let selection = snapshot(state, seat, kind);
                offers::to_control_device(&id, kind, selection.as_ref());
            }
        }
        manager::Request::Destroy => {}
    }
}

pub(crate) fn device_request<A: Access>(
    state: &mut A,
    device: ExtDataControlDeviceV1,
    request: device::Request,
) {
    let (kind, source) = match request {
        device::Request::SetSelection { source } => (SelectionKind::Clipboard, source),
        device::Request::SetPrimarySelection { source } => (SelectionKind::Primary, source),
        device::Request::Destroy => return,
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
                return device.post_error(device::Error::UsedSource);
            }
            record.freeze();
            Some(id)
        }
        None => None,
    };
    let Some(seat) = live_seat(state, device.key::<SeatId>()) else {
        if let Some(source) = source {
            source.cancelled();
        }
        return;
    };
    let serial = state.display().serial();
    publish::replace(state, seat, kind, id, serial);
}

pub(crate) fn device_destroyed<A: Access>(state: &mut A, device: ExtDataControlDeviceV1) {
    let Some(seat) = device.key::<SeatId>() else {
        return;
    };
    if let Some(entry) = state.selections().seats.get_mut(seat) {
        entry
            .control_devices
            .retain(|control_device| *control_device != device);
    }
}

pub(crate) fn source_request<A: Access>(
    state: &mut A,
    source: ExtDataControlSourceV1,
    request: source::Request,
) {
    let source::Request::Offer { mime_type } = request else {
        return;
    };
    let Some(id) = source.key::<SourceId>() else {
        return;
    };
    let Some(record) = state.selections().sources.get_mut(id) else {
        return;
    };
    if !record.is_fresh() {
        return source.post_error(source::Error::InvalidOffer);
    }
    record.offer(mime_type);
}

pub(crate) fn source_destroyed<A: Access>(state: &mut A, source: ExtDataControlSourceV1) {
    if let Some(id) = source.key::<SourceId>() {
        publish::source_gone(state, id);
    }
}

pub(crate) fn offer_request<A: Access>(
    state: &mut A,
    offer: ExtDataControlOfferV1,
    request: offer::Request,
) {
    if let offer::Request::Receive { mime_type, fd } = request
        && let Some(source) = offer.key::<SourceId>()
    {
        sources::deliver(state, source, mime_type, fd);
    }
}
