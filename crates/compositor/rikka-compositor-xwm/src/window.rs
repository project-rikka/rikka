use bitflags::bitflags;
pub use rikka_compositor_types::XWindowId;

use crate::geometry::Geometry;
use crate::hints::{SizeHints, WmHints};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowType {
    Normal,
    Dialog,
    Utility,
    Toolbar,
    Splash,
    Menu,
    DropdownMenu,
    PopupMenu,
    Tooltip,
    Notification,
    Combo,
    Dnd,
    Desktop,
    Dock,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct WindowStates: u16 {
        const MODAL = 1;
        const STICKY = 1 << 1;
        const MAXIMIZED_VERT = 1 << 2;
        const MAXIMIZED_HORZ = 1 << 3;
        const SHADED = 1 << 4;
        const SKIP_TASKBAR = 1 << 5;
        const SKIP_PAGER = 1 << 6;
        const HIDDEN = 1 << 7;
        const FULLSCREEN = 1 << 8;
        const ABOVE = 1 << 9;
        const BELOW = 1 << 10;
        const DEMANDS_ATTENTION = 1 << 11;
        const FOCUSED = 1 << 12;
        const MAXIMIZED = Self::MAXIMIZED_VERT.bits() | Self::MAXIMIZED_HORZ.bits();
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct Protocols: u8 {
        const TAKE_FOCUS = 1;
        const DELETE_WINDOW = 1 << 1;
        const PING = 1 << 2;
        const SYNC_REQUEST = 1 << 3;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputModel {
    NoInput,
    Passive,
    LocallyActive,
    GloballyActive,
}

impl InputModel {
    pub(crate) fn from_hints(hints: &WmHints, protocols: Protocols) -> Self {
        match (
            hints.input.unwrap_or(true),
            protocols.contains(Protocols::TAKE_FOCUS),
        ) {
            (false, false) => Self::NoInput,
            (true, false) => Self::Passive,
            (true, true) => Self::LocallyActive,
            (false, true) => Self::GloballyActive,
        }
    }

    pub(crate) fn sets_input_focus(self) -> bool {
        matches!(self, Self::Passive | Self::LocallyActive)
    }

    pub(crate) fn takes_focus(self) -> bool {
        matches!(self, Self::LocallyActive | Self::GloballyActive)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct XWindow {
    pub id: XWindowId,
    pub override_redirect: bool,
    pub mapped: bool,
    pub geometry: Geometry,
    pub title: String,
    pub class: String,
    pub instance: String,
    pub startup_id: Option<String>,
    pub pid: Option<u32>,
    pub transient_for: Option<XWindowId>,
    pub declared_types: Vec<WindowType>,
    pub hints: WmHints,
    pub size_hints: SizeHints,
    pub protocols: Protocols,
    pub states: WindowStates,
    pub decorated: bool,
    pub opacity: Option<f32>,
    pub surface_serial: Option<u64>,
}

impl XWindow {
    pub(crate) fn new(id: XWindowId, geometry: Geometry, override_redirect: bool) -> Self {
        Self {
            id,
            override_redirect,
            mapped: false,
            geometry,
            title: String::new(),
            class: String::new(),
            instance: String::new(),
            startup_id: None,
            pid: None,
            transient_for: None,
            declared_types: Vec::new(),
            hints: WmHints::default(),
            size_hints: SizeHints::default(),
            protocols: Protocols::empty(),
            states: WindowStates::empty(),
            decorated: true,
            opacity: None,
            surface_serial: None,
        }
    }

    #[must_use]
    pub fn window_type(&self) -> WindowType {
        match self.declared_types.first() {
            Some(declared) => *declared,
            None if self.transient_for.is_some() => WindowType::Dialog,
            None => WindowType::Normal,
        }
    }

    #[must_use]
    pub fn input_model(&self) -> InputModel {
        InputModel::from_hints(&self.hints, self.protocols)
    }

    #[must_use]
    pub fn is_maximized(&self) -> bool {
        self.states.contains(WindowStates::MAXIMIZED)
    }

    #[must_use]
    pub fn is_fullscreen(&self) -> bool {
        self.states.contains(WindowStates::FULLSCREEN)
    }

    #[must_use]
    pub fn is_minimized(&self) -> bool {
        self.states.contains(WindowStates::HIDDEN)
    }

    pub(crate) fn same_client(&self, other: XWindowId, client_mask: u32) -> bool {
        self.id.raw() & !client_mask == other.raw() & !client_mask
    }
}
