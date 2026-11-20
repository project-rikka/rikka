use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_protocols::wl_data_source::WlDataSource;
use rikka_compositor_protocols::wl_output::WlOutput;
use rikka_compositor_protocols::wl_seat::WlSeat;
use rikka_compositor_protocols::xdg_toplevel::ResizeEdge;
use rikka_compositor_surface::SurfaceId;
use rikka_wayland_wire::ClientId;

use crate::ids::{LayerId, LockId, LockSurfaceId, PopupId, ToplevelId};
use crate::xdg::ActivationToken;

#[derive(Debug)]
pub enum ShellEvent {
    Pong {
        client: ClientId,
    },
    Unresponsive {
        client: ClientId,
    },
    Toplevel {
        toplevel: ToplevelId,
        event: ToplevelEvent,
    },
    Popup {
        popup: PopupId,
        event: PopupEvent,
    },
    Layer {
        layer: LayerId,
        event: LayerEvent,
    },
    Lock(LockEvent),
    Activate {
        token: ActivationToken,
        surface: SurfaceId,
    },
    Bell {
        surface: Option<SurfaceId>,
    },
    XwaylandAssociated {
        surface: SurfaceId,
        serial: u64,
    },
}

#[derive(Debug)]
pub enum ToplevelEvent {
    Created,
    InitialCommit,
    Mapped,
    Unmapped,
    Destroyed,
    Configured {
        serial: u32,
    },
    Title,
    AppId,
    Parent,
    Decoration,
    Modal,
    Icon,
    Tag,
    Description,
    Move {
        seat: WlSeat,
        serial: u32,
    },
    Resize {
        seat: WlSeat,
        serial: u32,
        edges: ResizeEdge,
    },
    WindowMenu {
        seat: WlSeat,
        serial: u32,
        at: Point<i32, Logical>,
    },
    Maximize,
    Unmaximize,
    Fullscreen {
        output: Option<WlOutput>,
    },
    Unfullscreen,
    Minimize,
    DragAttached {
        source: WlDataSource,
        offset: Point<i32, Logical>,
    },
}

#[derive(Debug)]
pub enum PopupEvent {
    Created,
    InitialCommit,
    Mapped,
    Unmapped,
    Destroyed,
    Configured { serial: u32 },
    Grab { seat: WlSeat, serial: u32 },
    Reposition { token: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerEvent {
    Created,
    InitialCommit,
    Mapped,
    Unmapped,
    Destroyed,
    Configured { serial: u32 },
    Changed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockEvent {
    Requested(LockId),
    Abandoned(LockId),
    Unlocked(LockId),
    LockerGone(LockId),
    SurfaceCreated(LockSurfaceId),
    SurfaceDestroyed(LockSurfaceId),
}
