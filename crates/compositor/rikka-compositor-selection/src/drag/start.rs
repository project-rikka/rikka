use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_protocols::wl_data_device::{Error, WlDataDevice};
use rikka_compositor_protocols::wl_data_source::WlDataSource;
use rikka_compositor_protocols::wl_surface::WlSurface;
use rikka_compositor_seat::{Focus, GrabFocus, SeatId};
use rikka_wayland_wire::ClientId;

use crate::access::Access;
use crate::drag::end::cancel_source;
use crate::drag::focus::retarget;
use crate::drag::grab::{DragGrab, TouchDragGrab};
use crate::drag::icon;
use crate::drag::session::{Drag, DragDevice};
use crate::drag::toplevel;
use crate::event::SelectionEvent;
use crate::host::SelectionHost;
use crate::ids::SourceId;

#[derive(Debug)]
pub(crate) struct DragRequest {
    pub(crate) source: Option<WlDataSource>,
    pub(crate) origin: WlSurface,
    pub(crate) icon: Option<WlSurface>,
    pub(crate) serial: u32,
}

pub(crate) fn start<S: SelectionHost>(
    state: &mut S,
    device: &WlDataDevice,
    seat: SeatId,
    request: DragRequest,
) {
    let source = match &request.source {
        Some(source) => {
            let Some(id) = source.key::<SourceId>() else {
                return;
            };
            let Some(record) = state.selections().sources.get(id) else {
                return;
            };
            if !record.is_fresh() {
                return device.post_error(Error::UsedSource);
            }
            Some(id)
        }
        None => None,
    };
    let client = device.client();
    let dragging = state.selections().seat_mut(seat).drag.is_some();
    let Some(grab) = implicit_grab(state, seat, request.serial, client)
        .filter(|_| !dragging && request.origin.client() == client)
    else {
        return refuse(&request);
    };
    let icon = match &request.icon {
        Some(icon) => match icon::assign(state, icon) {
            Ok(icon) => Some(icon),
            Err(error) => return error.post(device, Error::Role.code()),
        },
        None => None,
    };
    if let Some(id) = source {
        state.selections().sources[id].freeze();
    }
    let tethered = request
        .source
        .as_ref()
        .is_some_and(|source| state.shell().toplevel_drag_started(source));
    let mut drag = Drag::new(source, Some(client), grab.device, grab.location, icon);
    drag.tethered = tethered;
    state.selections().seat_mut(seat).drag = Some(drag);
    match grab.device {
        DragDevice::Pointer => rikka_compositor_seat::seat(state, seat)
            .set_pointer_grab(DragGrab { seat }, GrabFocus::Clear),
        DragDevice::Touch { slot } => rikka_compositor_seat::seat(state, seat)
            .set_touch_grab(TouchDragGrab { seat, slot }, GrabFocus::Clear),
    }
    state.report(SelectionEvent::DragStarted { seat });
    retarget(state, seat, grab.location, grab.under);
    toplevel::follow(state, seat, grab.location);
}

pub(crate) fn refuse(request: &DragRequest) {
    if let Some(source) = &request.source {
        cancel_source(source);
    }
}

struct ImplicitGrab {
    device: DragDevice,
    location: Point<f64, Logical>,
    under: Option<Focus>,
}

fn implicit_grab<A: Access>(
    state: &mut A,
    seat: SeatId,
    serial: u32,
    client: ClientId,
) -> Option<ImplicitGrab> {
    let entry = state.seats().get(seat)?;
    let pointer = entry.pointer();
    let (grab, origin) = match pointer.grab_start(serial) {
        Some(start) => (
            ImplicitGrab {
                device: DragDevice::Pointer,
                location: pointer.location(),
                under: pointer.focus(),
            },
            start.focus?.surface,
        ),
        None => {
            let start = entry.touch().grab_start(serial)?;
            (
                ImplicitGrab {
                    device: DragDevice::Touch { slot: start.slot },
                    location: start.location,
                    under: Some(start.focus),
                },
                start.focus.surface,
            )
        }
    };
    let owner = state.surfaces().get(origin)?.client();
    (owner == client).then_some(grab)
}
