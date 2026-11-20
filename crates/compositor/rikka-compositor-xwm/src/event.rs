use std::os::fd::OwnedFd;

use rikka_types::{DragAction, ResizeEdge};

use crate::geometry::Geometry;
use crate::selection::{MimeList, SelectionKind};
use crate::window::XWindowId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConfigureSerial(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StackRequest {
    Top,
    Bottom,
    Above(XWindowId),
    Below(XWindowId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowProperty {
    Title,
    Class,
    Pid,
    StartupId,
    Hints,
    SizeHints,
    Protocols,
    TransientFor,
    WindowType,
    Decorations,
    Opacity,
    States,
}

#[derive(Debug)]
pub enum XwmEvent {
    MapRequest {
        window: XWindowId,
    },
    Mapped {
        window: XWindowId,
    },
    Unmapped {
        window: XWindowId,
    },
    Destroyed {
        window: XWindowId,
    },
    SurfaceSerial {
        window: XWindowId,
        serial: u64,
    },
    ConfigureRequest {
        window: XWindowId,
        x: Option<i32>,
        y: Option<i32>,
        width: Option<u32>,
        height: Option<u32>,
        stack: Option<StackRequest>,
    },
    ConfigureAccepted {
        window: XWindowId,
        serial: ConfigureSerial,
        geometry: Geometry,
    },
    Configured {
        window: XWindowId,
        geometry: Geometry,
    },
    PropertyChanged {
        window: XWindowId,
        property: WindowProperty,
    },
    MaximizeRequest {
        window: XWindowId,
        maximized: bool,
    },
    FullscreenRequest {
        window: XWindowId,
        fullscreen: bool,
    },
    MinimizeRequest {
        window: XWindowId,
        minimized: bool,
    },
    AboveRequest {
        window: XWindowId,
        above: bool,
    },
    FrameExtentsRequest {
        window: XWindowId,
    },
    ActivateRequest {
        window: XWindowId,
    },
    MoveRequest {
        window: XWindowId,
        button: u32,
    },
    ResizeRequest {
        window: XWindowId,
        button: u32,
        edge: ResizeEdge,
    },
    SelectionOffered {
        kind: SelectionKind,
        mime_types: MimeList,
    },
    SelectionCleared {
        kind: SelectionKind,
    },
    SendSelection {
        kind: SelectionKind,
        mime_type: String,
        fd: OwnedFd,
    },
    X11DragOwner {
        owned: bool,
    },
    X11DragEntered {
        mime_types: MimeList,
    },
    X11DragLeft,
    X11DragDropped,
    DragStatus {
        accepted: bool,
        action: Option<DragAction>,
    },
    DragFinished {
        accepted: bool,
        action: Option<DragAction>,
    },
}
