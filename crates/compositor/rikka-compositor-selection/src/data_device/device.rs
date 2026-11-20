use rikka_compositor_protocols::wl_data_device::{Error, Request, WlDataDevice};
use rikka_compositor_protocols::wl_data_source::{self, WlDataSource};
use rikka_compositor_seat::SeatId;
use rikka_compositor_server::Client;

use crate::access::Access;
use crate::admission::{admits, live_seat};
use crate::drag::{self, DragRequest};
use crate::host::SelectionHost;
use crate::ids::SourceId;
use crate::kind::SelectionKind;
use crate::publish;

pub(crate) fn device_request<S: SelectionHost>(
    state: &mut S,
    device: WlDataDevice,
    request: Request,
) {
    let seat = live_seat(state, device.key::<SeatId>());
    match request {
        Request::SetSelection { source, serial } => {
            if let Some(seat) = seat {
                set_selection(state, &device, seat, source, serial);
            }
        }
        Request::StartDrag {
            source,
            origin,
            icon,
            serial,
        } => {
            let request = DragRequest {
                source,
                origin,
                icon,
                serial,
            };
            match seat {
                Some(seat) => drag::start(state, &device, seat, request),
                None => drag::refuse(&request),
            }
        }
        Request::Release => {}
    }
}

pub(crate) fn device_destroyed<A: Access>(state: &mut A, device: WlDataDevice) {
    let Some(seat) = device.key::<SeatId>() else {
        return;
    };
    if let Some(entry) = state.selections().seats.get_mut(seat) {
        entry.data_devices.remove(&device);
    }
}

fn set_selection<S: SelectionHost>(
    state: &mut S,
    device: &WlDataDevice,
    seat: SeatId,
    source: Option<WlDataSource>,
    serial: u32,
) {
    let Some(client) = Client::from_resource(device) else {
        return;
    };
    let id = match &source {
        Some(source) => {
            let Some(id) = source.key::<SourceId>() else {
                return;
            };
            let Some(record) = state.selections().sources.get(id) else {
                return;
            };
            let (fresh, has_actions) = (record.is_fresh(), record.actions.is_some());
            if !fresh {
                if !publish::renew(state, seat, SelectionKind::Clipboard, id) {
                    device.post_error(Error::UsedSource);
                }
                return;
            }
            if has_actions {
                return source.post_error(wl_data_source::Error::InvalidSource);
            }
            Some(id)
        }
        None => None,
    };
    if let Some(id) = id {
        state.selections().sources[id].freeze();
    }
    if !admits(state, &client, seat, SelectionKind::Clipboard, serial) {
        if let Some(source) = source {
            source.cancelled();
        }
        return;
    }
    publish::replace(state, seat, SelectionKind::Clipboard, id, serial);
}
