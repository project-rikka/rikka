use rikka_input::DeviceEvent;
use rikka_types::Stylus;
use rikka_wayland_client::wl_surface::WlSurface;

use crate::pad::PadEvent;

#[derive(Debug)]
pub enum SeatEvent {
    Input {
        surface: WlSurface,
        event: DeviceEvent,
    },
    Stylus {
        surface: WlSurface,
        event: DeviceEvent,
        stylus: Stylus,
    },
    Keyboard {
        surface: WlSurface,
        entered: bool,
    },
    Pad {
        surface: WlSurface,
        event: PadEvent,
    },
}
