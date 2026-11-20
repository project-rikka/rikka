use rikka_wayland::Display;
use rikka_wayland_client::Route;
use rikka_wayland_client::wl_seat::WlSeat;
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::zwp_keyboard_shortcuts_inhibit_manager_v1::ZwpKeyboardShortcutsInhibitManagerV1;
use rikka_wayland_client::zwp_keyboard_shortcuts_inhibitor_v1::ZwpKeyboardShortcutsInhibitorV1;

const SEAT: u32 = 1;

#[derive(Debug)]
pub(super) struct HostShortcuts {
    seats: Vec<WlSeat>,
    inhibitors: Vec<ZwpKeyboardShortcutsInhibitorV1>,
}

impl HostShortcuts {
    pub(super) fn hold(display: &Display, surface: &WlSurface) -> Option<HostShortcuts> {
        let manager: ZwpKeyboardShortcutsInhibitManagerV1 = display.bind_optional(1..=1)?;
        let seats: Vec<WlSeat> = display.bind_all(&Route::discard(), SEAT);
        let inhibitors = seats
            .iter()
            .map(|seat| manager.inhibit_shortcuts(surface, seat))
            .collect();
        manager.destroy();
        tracing::info!("host shortcuts inhibited while focused");
        Some(HostShortcuts { seats, inhibitors })
    }
}

impl Drop for HostShortcuts {
    fn drop(&mut self) {
        for inhibitor in self.inhibitors.drain(..) {
            inhibitor.destroy();
        }
        self.seats.clear();
    }
}
