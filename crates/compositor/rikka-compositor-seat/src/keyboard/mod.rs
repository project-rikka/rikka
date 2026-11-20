mod deliver;
mod focus;
mod inhibit;
mod keymap;
mod keystroke;
mod modifiers;
mod present;
mod protocol;
mod route;
mod xkb_state;
mod xwayland;

use std::fmt;

use rikka_compositor_protocols::wl_keyboard::WlKeyboard;
use rikka_compositor_protocols::wl_surface::WlSurface;
use rikka_compositor_server::ByClient;
use rikka_compositor_surface::SurfaceId;
use rikka_wayland_wire::{ClientId, serial_not_before};

use crate::focus::KeyboardFocus;

pub(crate) use deliver::introduce;
pub(crate) use focus::{refocus_after_destroy, restore, set_focus, withhold};
pub(crate) use inhibit::{inhibit_manager, inhibitor, inhibitor_destroyed};
pub use keymap::{Keymap, KeymapNames};
pub use keystroke::{KeyFilter, Keystroke};
pub use modifiers::{Modifiers, SerializedModifiers};
pub(crate) use present::{SentKeyboardState, reselect, set_repeat};
pub(crate) use protocol::{keyboard_destroyed, keyboard_request};
pub(crate) use route::{forget_virtual, key, virtual_modifiers};
pub(crate) use xkb_state::Origin;
pub(crate) use xwayland::{xwayland_grab, xwayland_grab_destroyed, xwayland_manager};

use inhibit::Inhibitor;
use xkb_state::XkbState;
use xwayland::XwaylandGrab;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyboardGrab {
    InputMethod,
    Xwayland { surface: SurfaceId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Repeat {
    pub rate: i32,
    pub delay: i32,
}

impl Default for Repeat {
    fn default() -> Self {
        Self {
            rate: 25,
            delay: 600,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Route {
    Host,
    InputMethod,
    Client,
}

pub struct Keyboard {
    pub(crate) keyboards: ByClient<WlKeyboard>,
    pub(crate) default: Keymap,
    pub(crate) physical: XkbState,
    pub(crate) virtuals: Vec<XkbState>,
    pub(crate) active: Origin,
    pub(crate) sent: SentKeyboardState,
    pub(crate) focus: Option<KeyboardFocus>,
    pub(crate) target: Option<WlSurface>,
    pub(crate) entered: Option<u32>,
    pub(crate) withheld: bool,
    pub(crate) repeat: Repeat,
    pub(crate) inhibitors: Vec<Inhibitor>,
    pub(crate) xwayland: Option<XwaylandGrab>,
}

impl Keyboard {
    pub(crate) fn new(keymap: Keymap) -> Self {
        let physical = XkbState::new(Origin::Devices, keymap.clone());
        Self {
            keyboards: ByClient::default(),
            sent: SentKeyboardState {
                origin: Origin::Devices,
                keymap: keymap.clone(),
                modifiers: physical.serialized(),
            },
            default: keymap,
            physical,
            virtuals: Vec::new(),
            active: Origin::Devices,
            focus: None,
            target: None,
            entered: None,
            withheld: false,
            repeat: Repeat::default(),
            inhibitors: Vec::new(),
            xwayland: None,
        }
    }

    pub fn focus(&self) -> Option<KeyboardFocus> {
        self.focus
    }

    pub fn entered(&self) -> Option<u32> {
        self.entered
    }

    pub fn issued_since_enter(&self, serial: u32) -> bool {
        self.entered
            .is_some_and(|entered| serial_not_before(serial, entered))
    }

    pub fn keymap(&self) -> &Keymap {
        &self.sent.keymap
    }

    pub fn modifiers(&self) -> Modifiers {
        self.xkb_state(self.active)
            .map(XkbState::modifiers)
            .unwrap_or_default()
    }

    pub fn held_modifiers(&self) -> Modifiers {
        std::iter::once(&self.physical)
            .chain(&self.virtuals)
            .map(XkbState::modifiers)
            .fold(Modifiers::default(), |held, one| held | one)
    }

    pub fn repeat(&self) -> Repeat {
        self.repeat
    }

    pub fn shortcuts_inhibited(&self) -> bool {
        self.focus.is_some_and(|focus| {
            self.inhibitors
                .iter()
                .any(|inhibitor| inhibitor.surface == focus.surface())
        })
    }

    pub fn xwayland_grab(&self) -> Option<SurfaceId> {
        self.xwayland.as_ref().map(|grab| grab.surface)
    }

    pub(crate) fn xkb_state(&self, origin: Origin) -> Option<&XkbState> {
        match origin {
            Origin::Devices => Some(&self.physical),
            Origin::Virtual(_) => self
                .virtuals
                .iter()
                .find(|xkb_state| xkb_state.origin == origin),
        }
    }

    pub(crate) fn xkb_state_mut(&mut self, origin: Origin) -> Option<&mut XkbState> {
        match origin {
            Origin::Devices => Some(&mut self.physical),
            Origin::Virtual(_) => self
                .virtuals
                .iter_mut()
                .find(|xkb_state| xkb_state.origin == origin),
        }
    }

    pub(crate) fn focused_client(&self) -> Option<ClientId> {
        self.target
            .as_ref()
            .filter(|_| !self.withheld)
            .map(WlSurface::client)
    }

    pub(crate) fn delivered(&self) -> Vec<u32> {
        std::iter::once(&self.physical)
            .chain(&self.virtuals)
            .flat_map(|xkb_state| &xkb_state.pressed)
            .filter(|pressed| pressed.route == Route::Client)
            .map(|pressed| pressed.code)
            .collect()
    }
}

impl fmt::Debug for Keyboard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Keyboard")
            .field("keymap", &self.sent.keymap)
            .field("sent", &self.sent.modifiers)
            .field("active", &self.active)
            .field("focus", &self.focus)
            .field("repeat", &self.repeat)
            .field("inhibitors", &self.inhibitors.len())
            .field("xwayland", &self.xwayland)
            .finish_non_exhaustive()
    }
}
