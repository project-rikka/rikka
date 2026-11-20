use std::os::fd::OwnedFd;

use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_protocols::wl_data_device_manager::DndAction;
use rikka_compositor_seat::SeatId;
use rikka_compositor_shell::ToplevelId;
use rikka_compositor_surface::SurfaceId;

use crate::ids::{CompositorSource, ForeignOffer};
use crate::kind::SelectionKind;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SelectionEvent {
    Changed {
        seat: SeatId,
        kind: SelectionKind,
    },
    DragStarted {
        seat: SeatId,
    },
    DragIconChanged {
        seat: SeatId,
    },
    DragAccepted {
        seat: SeatId,
    },
    DragToplevelMoved {
        seat: SeatId,
        toplevel: ToplevelId,
        at: Point<f64, Logical>,
    },
    DragToplevelReleased {
        seat: SeatId,
        toplevel: ToplevelId,
    },
    DragEnded {
        seat: SeatId,
        dropped: bool,
    },
    ForeignDrag {
        seat: SeatId,
        offer: ForeignOffer,
        event: ForeignDragEvent,
    },
    CompositorDrag {
        seat: SeatId,
        source: CompositorSource,
        event: CompositorDragEvent,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ForeignDragEvent {
    Entered {
        surface: SurfaceId,
        at: Point<f64, Logical>,
    },
    Moved {
        at: Point<f64, Logical>,
    },
    Left,
    Dropped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositorDragEvent {
    Target(bool),
    Action(DndAction),
    Finished(Option<DndAction>),
    Cancelled,
}

#[derive(Debug)]
pub struct SendSelection {
    pub seat: SeatId,
    pub kind: SelectionKind,
    pub source: CompositorSource,
    pub mime_type: String,
    pub fd: OwnedFd,
}

#[derive(Debug)]
pub struct SendDrag {
    pub seat: SeatId,
    pub source: CompositorSource,
    pub mime_type: String,
    pub fd: OwnedFd,
}
