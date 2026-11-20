use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_protocols::wl_data_device::WlDataDevice;
use rikka_compositor_protocols::wl_data_device_manager::DndAction;
use rikka_compositor_seat::{Focus, Motion, SeatId};

use crate::access::Access;
use crate::drag::feedback::Feedback;
use crate::drag::foreign;
use crate::drag::negotiate::renegotiate;
use crate::drag::offer::{self, Stage};
use crate::drag::session::{Target, drag_mut};
use crate::drag::toplevel;
use crate::event::{ForeignDragEvent, SelectionEvent};
use crate::source::SourceRecord;

pub(crate) fn moved<A: Access>(state: &mut A, seat: SeatId, motion: &Motion) {
    let current = drag_mut(state, seat).and_then(|drag| {
        drag.at = motion.location;
        drag.target.as_ref().map(|target| target.surface)
    });
    let under = motion
        .under
        .filter(|under| state.surfaces().get(under.surface).is_some());
    match under {
        Some(under) if current == Some(under.surface) => {
            let Some(target) = drag_mut(state, seat).and_then(|drag| drag.target.as_ref()) else {
                return;
            };
            if !target.foreign {
                let local = under.local(motion.location);
                let time = motion.time.as_millis() as u32;
                for device in target.devices.iter().filter(|device| device.alive()) {
                    device.motion(time, local.x, local.y);
                }
            } else if let Some(offer) = target.offer {
                foreign::emit(
                    state,
                    seat,
                    offer,
                    ForeignDragEvent::Moved {
                        at: motion.location,
                    },
                );
            }
        }
        _ => retarget(state, seat, motion.location, under),
    }
    toplevel::follow(state, seat, motion.location);
}

pub(crate) fn retarget<A: Access>(
    state: &mut A,
    seat: SeatId,
    location: Point<f64, Logical>,
    under: Option<Focus>,
) {
    if leave_target(state, seat) {
        withdraw_feedback(state, seat);
    }
    if let Some(under) = under {
        enter(state, seat, under, location);
    }
}

pub(crate) fn leave_target<A: Access>(state: &mut A, seat: SeatId) -> bool {
    let Some(target) = drag_mut(state, seat).and_then(|drag| drag.target.take()) else {
        return false;
    };
    for device in target.devices.iter().filter(|device| device.alive()) {
        device.leave();
    }
    if let Some(id) = target.offer
        && let Some(record) = state.selections().drag_offers.get_mut(id)
        && record.stage == Stage::Hovering
    {
        let accepted = record.accepted;
        record.stage = Stage::Left;
        if accepted {
            state.report(SelectionEvent::DragAccepted { seat });
        }
        if target.foreign {
            foreign::emit(state, seat, id, ForeignDragEvent::Left);
            state.selections().drag_offers.remove(id);
        }
    }
    true
}

fn withdraw_feedback<A: Access>(state: &mut A, seat: SeatId) {
    let Some(source) = drag_mut(state, seat).and_then(|drag| drag.source) else {
        return;
    };
    let Some(feedback) = Feedback::for_source(state, source) else {
        return;
    };
    feedback.target(state, None);
    let Some(drag) = drag_mut(state, seat) else {
        return;
    };
    if !drag.action.is_empty() {
        drag.action = DndAction::empty();
        feedback.action(state, DndAction::empty());
    }
}

fn enter<A: Access>(state: &mut A, seat: SeatId, under: Focus, location: Point<f64, Logical>) {
    let Some(resource) = state
        .surfaces()
        .get(under.surface)
        .map(|surface| surface.resource().clone())
        .filter(|resource| resource.alive())
    else {
        return;
    };
    let client = resource.client();
    let Some(drag) = drag_mut(state, seat) else {
        return;
    };
    let (source, origin) = (drag.source, drag.origin);
    let bridged = source.is_some_and(|source| {
        state
            .selections()
            .sources
            .get(source)
            .is_some_and(SourceRecord::is_dragged)
    });
    if state.foreign_drag_target(under.surface) {
        if let Some(source) = source.filter(|_| !bridged) {
            foreign::enter(state, seat, source, under.surface, location);
        }
        return;
    }
    let devices: Vec<WlDataDevice> = if source.is_some() || Some(client) == origin {
        state
            .selections()
            .seat_mut(seat)
            .data_devices
            .for_client(client)
            .iter()
            .filter(|device| device.alive())
            .cloned()
            .collect()
    } else {
        Vec::new()
    };
    let created = match source {
        Some(source) if !devices.is_empty() => offer::create(state, seat, source, &devices),
        _ => None,
    };
    let serial = state.display().next_serial();
    let local = under.local(location);
    let offer = created.as_ref().map(|(id, _)| *id);
    if let Some(id) = offer {
        renegotiate(state, id);
    }
    match (&created, source) {
        (Some((_, pairs)), _) => {
            for (device, object) in pairs {
                device.enter(serial, &resource, local.x, local.y, Some(object));
            }
        }
        (None, Some(_)) => {}
        (None, None) => {
            for device in &devices {
                device.enter(serial, &resource, local.x, local.y, None);
            }
        }
    }
    let entered = match created {
        Some((_, pairs)) => pairs.into_iter().map(|(device, _)| device).collect(),
        None if source.is_none() => devices,
        None => Vec::new(),
    };
    if let Some(drag) = drag_mut(state, seat) {
        drag.target = Some(Target {
            surface: under.surface,
            devices: entered,
            offer,
            foreign: false,
        });
    }
}
