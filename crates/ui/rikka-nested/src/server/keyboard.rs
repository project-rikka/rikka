use std::time::Instant;

use rikka_compositor_seat::{
    InputDevice, Key, KeySource, KeyboardFocus, Keymap, KeymapNames, Repeat,
};
use rikka_compositor_types::Press;
use rikka_input::xkb::SessionKeyboard;

use super::state::State;
use crate::view::ViewId;

const DEVICE: InputDevice = InputDevice(0);

#[derive(Debug, Default)]
pub(crate) struct Keyboard {
    pub(crate) focused: Option<ViewId>,
    pub(crate) last_focused: Option<ViewId>,
    generation: u64,
    pressed: Vec<u32>,
}

pub(crate) fn initial_keymap() -> Result<Keymap, String> {
    let published = SessionKeyboard::current();
    if let Some(text) = published.as_ref().and_then(SessionKeyboard::keymap) {
        match Keymap::from_text(text) {
            Ok(keymap) => return Ok(keymap),
            Err(error) => tracing::warn!("failed to compile session keymap: {error}"),
        }
    }
    Keymap::from_names(&KeymapNames::default()).map_err(|error| error.to_string())
}

pub(crate) fn follow_session(state: &mut State) {
    let Some(published) = SessionKeyboard::current() else {
        return;
    };
    if published.generation() == state.keyboard.generation {
        return;
    }
    state.keyboard.generation = published.generation();
    let seat = state.seat;
    let mut seat_handle = rikka_compositor_seat::seat(state, seat);
    if let Some(repeat) = published.repeat() {
        seat_handle.set_repeat(Repeat {
            rate: repeat.rate,
            delay: repeat.delay,
        });
    }
    if let Some(text) = published.keymap() {
        match Keymap::from_text(text) {
            Ok(keymap) => seat_handle.set_keymap(keymap),
            Err(error) => tracing::warn!("failed to compile session keymap: {error}"),
        }
    }
}

impl State {
    pub(crate) fn key(&mut self, view: ViewId, code: u32, pressed: bool, at: Instant) {
        if self.keyboard.focused != Some(view) {
            return;
        }
        follow_session(self);
        let was_pressed = self.keyboard.pressed.contains(&code);
        match (pressed, was_pressed) {
            (true, true) | (false, false) => return,
            (true, false) => self.keyboard.pressed.push(code),
            (false, true) => self.keyboard.pressed.retain(|known| *known != code),
        }
        let press = match pressed {
            true => Press::Pressed,
            false => Press::Released,
        };
        let time = self.input_time(at);
        let seat = self.seat;
        rikka_compositor_seat::seat(self, seat).key(Key {
            code,
            press,
            time,
            source: KeySource::Device(DEVICE),
        });
    }

    pub(crate) fn focus(&mut self, view: ViewId, gained: bool, at: Instant) {
        match gained {
            true => {
                if let Some(previous) = self.keyboard.focused.filter(|previous| *previous != view) {
                    self.unfocus(previous, at);
                }
                follow_session(self);
                self.keyboard.focused = Some(view);
                self.keyboard.last_focused = Some(view);
                self.set_focused(view, true);
                self.refocus(view);
            }
            false if self.keyboard.focused == Some(view) => self.unfocus(view, at),
            false => {}
        }
    }

    fn unfocus(&mut self, view: ViewId, at: Instant) {
        for code in std::mem::take(&mut self.keyboard.pressed) {
            let time = self.input_time(at);
            let seat = self.seat;
            rikka_compositor_seat::seat(self, seat).key(Key {
                code,
                press: Press::Released,
                time,
                source: KeySource::Device(DEVICE),
            });
        }
        self.dismiss_grabs(view);
        self.keyboard.focused = None;
        self.focus_keyboard(None);
        self.set_focused(view, false);
    }

    pub(crate) fn refocus(&mut self, view: ViewId) {
        let target = self.keyboard_target(view);
        self.focus_keyboard(target);
    }

    pub(crate) fn focus_keyboard(&mut self, focus: Option<KeyboardFocus>) {
        let seat = self.seat;
        rikka_compositor_seat::seat(self, seat).set_keyboard_focus(focus);
    }
}
