use rikka_compositor_protocols::wl_data_device_manager::{Request, WlDataDeviceManager};
use rikka_compositor_seat::SeatId;

use crate::access::Access;
use crate::admission::{live_seat, root};
use crate::kind::SelectionKind;
use crate::offers;
use crate::publish::snapshot;
use crate::source::{Origin, SourceRecord};

pub(crate) fn manager_request<A: Access>(
    state: &mut A,
    _manager: WlDataDeviceManager,
    request: Request,
) {
    match request {
        Request::CreateDataSource { id } => {
            let source = state
                .selections()
                .sources
                .insert(SourceRecord::new(Origin::Data(id.clone())));
            id.set_key(source);
        }
        Request::GetDataDevice { id, seat } => {
            let Some(wire) = live_seat(state, seat.key::<SeatId>()) else {
                return;
            };
            id.set_key(wire);
            let seat = root(state, wire);
            let entry = state.selections().seat_mut(seat);
            entry.data_devices.add(id.clone());
            if entry.focused_since(id.client()).is_some() {
                let selection = snapshot(state, seat, SelectionKind::Clipboard);
                offers::to_data_device(&id, selection.as_ref());
            }
        }
        Request::Release => {}
    }
}
