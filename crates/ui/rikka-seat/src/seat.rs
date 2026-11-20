use rikka_types::Modifiers;
use rikka_wayland_client::wl_seat::{Capability, WlSeat};

use crate::cursor::Owner;
use crate::gestures::Gestures;
use crate::keyboard::Keyboard;
use crate::pointer::Pointer;
use crate::seats::Managers;
use crate::tablet::TabletSeat;
use crate::text_input::TextInput;
use crate::touch::Touch;

const RELEASE_SINCE_VERSION: u32 = 5;

#[derive(Debug)]
pub(crate) struct Seat {
    pub(crate) global: u32,
    pub(crate) seat: WlSeat,
    capabilities: Capability,
    pub(crate) keyboard: Option<Keyboard>,
    pub(crate) pointer: Option<Pointer>,
    pub(crate) touch: Option<Touch>,
    pub(crate) tablet: Option<TabletSeat>,
    pub(crate) text_input: Option<TextInput>,
    pub(crate) modifiers: Modifiers,
}

impl Seat {
    pub(crate) fn new(
        global: u32,
        seat: WlSeat,
        tablet: Option<TabletSeat>,
        text_input: Option<TextInput>,
    ) -> Seat {
        Seat {
            global,
            seat,
            capabilities: Capability::empty(),
            keyboard: None,
            pointer: None,
            touch: None,
            tablet,
            text_input,
            modifiers: Modifiers::default(),
        }
    }

    pub(crate) fn release(mut self) {
        self.drop_keyboard();
        self.drop_pointer();
        self.drop_touch();
        if let Some(tablet) = self.tablet.take() {
            tablet.release();
        }
        if let Some(text_input) = self.text_input.take() {
            text_input.release();
        }
        if self.seat.version() >= RELEASE_SINCE_VERSION {
            self.seat.release();
        }
    }

    pub(crate) fn set_capabilities(&mut self, capabilities: Capability, managers: &Managers) {
        let previous = std::mem::replace(&mut self.capabilities, capabilities);
        let (added, removed) = (capabilities - previous, previous - capabilities);
        if added.contains(Capability::KEYBOARD) {
            self.keyboard = Some(Keyboard::new(self.seat.get_keyboard()));
        } else if removed.contains(Capability::KEYBOARD) {
            self.drop_keyboard();
        }
        if added.contains(Capability::POINTER) {
            let pointer = self.seat.get_pointer();
            let cursor = managers.cursors.device(Owner::Pointer(&pointer));
            let gestures = managers
                .gestures
                .as_ref()
                .map(|manager| Gestures::new(manager, &pointer));
            let mut pointer = Pointer::new(pointer, cursor, gestures);
            if let Some(manager) = managers
                .relative
                .as_ref()
                .filter(|_| managers.relative_motion_enabled)
            {
                pointer.set_relative_motion(manager, true);
            }
            self.pointer = Some(pointer);
        } else if removed.contains(Capability::POINTER) {
            self.drop_pointer();
        }
        if added.contains(Capability::TOUCH) {
            self.touch = Some(Touch::new(self.seat.get_touch()));
        } else if removed.contains(Capability::TOUCH) {
            self.drop_touch();
        }
    }

    fn drop_keyboard(&mut self) {
        if let Some(keyboard) = self.keyboard.take() {
            keyboard.release();
        }
    }

    fn drop_pointer(&mut self) {
        if let Some(pointer) = self.pointer.take() {
            pointer.release();
        }
    }

    fn drop_touch(&mut self) {
        if let Some(touch) = self.touch.take() {
            touch.release();
        }
    }
}
