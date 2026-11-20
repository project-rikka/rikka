use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_protocols::wl_data_device::WlDataDevice;
use rikka_compositor_protocols::wl_data_device_manager::DndAction;
use rikka_compositor_protocols::wl_data_source::WlDataSource;
use rikka_compositor_seat::SeatId;
use rikka_compositor_shell::ToplevelId;
use rikka_compositor_surface::SurfaceId;
use rikka_wayland_wire::ClientId;

use slotmap::SlotMap;

use crate::access::Access;
use crate::drag::DragOffer;
use crate::drag::end::finish;
use crate::ids::{DragOfferId, SourceId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragDevice {
    Pointer,
    Touch { slot: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragView {
    pub device: DragDevice,
    pub at: Point<f64, Logical>,
    pub icon: Option<SurfaceId>,
    pub icon_offset: Point<i32, Logical>,
    pub target: Option<SurfaceId>,
    pub accepted: bool,
    pub toplevel: Option<ToplevelId>,
    pub driven: bool,
}

#[derive(Debug)]
pub(crate) struct Drag {
    pub(crate) source: Option<SourceId>,
    pub(crate) origin: Option<ClientId>,
    pub(crate) device: DragDevice,
    pub(crate) at: Point<f64, Logical>,
    pub(crate) icon: Option<SurfaceId>,
    pub(crate) icon_offset: Point<i32, Logical>,
    pub(crate) target: Option<Target>,
    pub(crate) action: DndAction,
    pub(crate) released: bool,
    pub(crate) toplevel: Option<ToplevelId>,
    pub(crate) driven: bool,
    pub(crate) tethered: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Target {
    pub(crate) surface: SurfaceId,
    pub(crate) devices: Vec<WlDataDevice>,
    pub(crate) offer: Option<DragOfferId>,
    pub(crate) foreign: bool,
}

impl Drag {
    pub(crate) fn new(
        source: Option<SourceId>,
        origin: Option<ClientId>,
        device: DragDevice,
        at: Point<f64, Logical>,
        icon: Option<SurfaceId>,
    ) -> Self {
        Self {
            source,
            origin,
            device,
            at,
            icon,
            icon_offset: Point::zero(),
            target: None,
            action: DndAction::empty(),
            released: false,
            toplevel: None,
            driven: false,
            tethered: false,
        }
    }

    pub(crate) fn view(&self, offers: &SlotMap<DragOfferId, DragOffer>) -> DragView {
        DragView {
            device: self.device,
            at: self.at,
            icon: self.icon,
            icon_offset: self.icon_offset,
            target: self.target.as_ref().map(|target| target.surface),
            accepted: self
                .target
                .as_ref()
                .and_then(|target| offers.get(target.offer?))
                .is_some_and(|offer| offer.accepted),
            toplevel: self.toplevel,
            driven: self.driven,
        }
    }
}

pub(crate) fn drag_mut<A: Access>(state: &mut A, seat: SeatId) -> Option<&mut Drag> {
    state.selections().seats.get_mut(seat)?.drag.as_mut()
}

pub(crate) fn drag_source<A: Access>(state: &mut A, seat: SeatId) -> Option<WlDataSource> {
    let selections = state.selections();
    let source = selections.seats.get(seat)?.drag.as_ref()?.source?;
    selections.sources.get(source)?.data_source().cloned()
}

pub(crate) fn end_drag_grab<A: Access>(state: &mut A, seat: SeatId) {
    let Some((device, driven)) = drag_mut(state, seat).map(|drag| (drag.device, drag.driven))
    else {
        return;
    };
    if driven {
        return finish(state, seat);
    }
    match device {
        DragDevice::Pointer => {
            rikka_compositor_seat::seat(state.seat_access(), seat).end_pointer_grab()
        }
        DragDevice::Touch { .. } => {
            rikka_compositor_seat::seat(state.seat_access(), seat).end_touch_grab()
        }
    }
}
