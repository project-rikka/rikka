use bitflags::bitflags;
use rikka_compositor_protocols::wl_seat::{self, WlSeat};
use rikka_compositor_surface::SurfaceId;
use rikka_wayland_wire::ClientId;

use crate::ime::Ime;
use crate::keyboard::{Keyboard, KeyboardGrab};
use crate::pointer::Pointer;
use crate::tablet::Tablets;
use crate::touch::Touch;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Capabilities: u32 {
        const POINTER = 1;
        const KEYBOARD = 1 << 1;
        const TOUCH = 1 << 2;
    }
}

impl Capabilities {
    pub(crate) fn wire(self) -> wl_seat::Capability {
        let mut wire = wl_seat::Capability::empty();
        wire.set(wl_seat::Capability::POINTER, self.contains(Self::POINTER));
        wire.set(wl_seat::Capability::KEYBOARD, self.contains(Self::KEYBOARD));
        wire.set(wl_seat::Capability::TOUCH, self.contains(Self::TOUCH));
        wire
    }
}

#[derive(Debug)]
pub struct Seat {
    pub(crate) name: String,
    pub(crate) capabilities: Capabilities,
    pub(crate) handles: Vec<WlSeat>,
    pub(crate) keyboard: Keyboard,
    pub(crate) pointer: Pointer,
    pub(crate) touch: Touch,
    pub(crate) tablets: Tablets,
    pub(crate) ime: Ime,
}

impl Seat {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    pub fn keyboard(&self) -> &Keyboard {
        &self.keyboard
    }

    pub fn pointer(&self) -> &Pointer {
        &self.pointer
    }

    pub fn touch(&self) -> &Touch {
        &self.touch
    }

    pub fn tablets(&self) -> &Tablets {
        &self.tablets
    }

    pub fn composing(&self) -> Option<SurfaceId> {
        self.ime.composing.surface()
    }

    pub fn keyboard_grab(&self) -> Option<KeyboardGrab> {
        match (self.keyboard.xwayland_grab(), &self.ime.grab) {
            (Some(surface), _) => Some(KeyboardGrab::Xwayland { surface }),
            (None, Some(_)) => Some(KeyboardGrab::InputMethod),
            (None, None) => None,
        }
    }

    pub(crate) fn input_method_client(&self) -> Option<ClientId> {
        self.ime
            .method
            .as_ref()
            .map(|method| method.resource.client())
    }
}
