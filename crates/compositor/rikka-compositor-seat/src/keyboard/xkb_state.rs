use rikka_compositor_types::Press;
use xkbcommon::xkb::{self, Keycode};

use crate::ids::VirtualKeyboardId;
use crate::input::KeySource;
use crate::keyboard::Route;
use crate::keyboard::keymap::Keymap;
use crate::keyboard::modifiers::{Modifiers, SerializedModifiers};

const MODIFIER_STATE: xkb::StateComponent = xkb::STATE_MODS_DEPRESSED
    | xkb::STATE_MODS_LATCHED
    | xkb::STATE_MODS_LOCKED
    | xkb::STATE_LAYOUT_EFFECTIVE;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Origin {
    Devices,
    Virtual(VirtualKeyboardId),
}

impl Origin {
    pub(crate) fn from_source(source: KeySource) -> Self {
        match source {
            KeySource::Device(_) => Self::Devices,
            KeySource::Virtual(board) | KeySource::InputMethod(board) => Self::Virtual(board),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PressedKey {
    pub(crate) code: u32,
    pub(crate) route: Route,
}

pub(crate) struct XkbState {
    pub(crate) origin: Origin,
    pub(crate) keymap: Keymap,
    pub(crate) state: xkb::State,
    pub(crate) pressed: Vec<PressedKey>,
    serialized: SerializedModifiers,
    modifiers: Modifiers,
}

impl XkbState {
    pub(crate) fn new(origin: Origin, keymap: Keymap) -> Self {
        let mut xkb_state = Self {
            origin,
            state: xkb::State::new(keymap.xkb()),
            keymap,
            pressed: Vec::new(),
            serialized: SerializedModifiers::default(),
            modifiers: Modifiers::default(),
        };
        xkb_state.read_modifiers();
        xkb_state
    }

    pub(crate) fn rekey(&mut self, keymap: &Keymap) {
        if self.keymap == *keymap {
            return;
        }
        let locked = self.serialized.locked;
        let mut next = xkb::State::new(keymap.xkb());
        for pressed in &self.pressed {
            next.update_key(Keycode::new(pressed.code + 8), xkb::KeyDirection::Down);
        }
        let pressed = SerializedModifiers::from_state(&next);
        SerializedModifiers { locked, ..pressed }.apply(&mut next);
        self.state = next;
        self.keymap = keymap.clone();
        self.read_modifiers();
    }

    pub(crate) fn update_key(&mut self, code: u32, press: Press) {
        let direction = match press {
            Press::Pressed => xkb::KeyDirection::Down,
            Press::Released => xkb::KeyDirection::Up,
        };
        let changed = self.state.update_key(Keycode::new(code + 8), direction);
        if changed & MODIFIER_STATE != 0 {
            self.read_modifiers();
        }
    }

    pub(crate) fn set_modifiers(&mut self, modifiers: SerializedModifiers) {
        if modifiers.apply(&mut self.state) & MODIFIER_STATE != 0 {
            self.read_modifiers();
        }
    }

    pub(crate) fn hold(&mut self, code: u32, route: Route) {
        self.pressed.push(PressedKey { code, route });
    }

    pub(crate) fn release(&mut self, code: u32) -> Option<Route> {
        let index = self
            .pressed
            .iter()
            .position(|pressed| pressed.code == code)?;
        Some(self.pressed.remove(index).route)
    }

    pub(crate) fn serialized(&self) -> SerializedModifiers {
        self.serialized
    }

    pub(crate) fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    fn read_modifiers(&mut self) {
        let serialized = SerializedModifiers::from_state(&self.state);
        self.serialized = serialized;
        self.modifiers = Modifiers::from_effective(
            serialized.depressed | serialized.latched | serialized.locked,
            self.keymap.masks(),
        );
    }
}
