use rikka_wayland::Host;
use rikka_wayland_client::wl_seat::WlSeat;
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::zwp_keyboard_shortcuts_inhibit_manager_v1::ZwpKeyboardShortcutsInhibitManagerV1;

use crate::wayland::inhibit::inhibitor::ShortcutInhibitor;

#[must_use]
#[derive(Debug)]
pub(crate) struct Shortcuts {
    host: Host,
    manager: ZwpKeyboardShortcutsInhibitManagerV1,
    seat: WlSeat,
}

impl Shortcuts {
    pub(crate) fn on(host: &Host) -> Option<Shortcuts> {
        let display = host.display();
        let seat = display
            .seat(None)
            .inspect_err(|error| tracing::debug!("no seat for shortcut inhibitor: {error}"))
            .ok()?;
        let manager = display.bind_optional(1..=1)?;
        Some(Shortcuts {
            host: host.clone(),
            manager,
            seat,
        })
    }

    pub(crate) fn inhibit(&self, surface: &WlSurface) -> ShortcutInhibitor {
        let inhibitor = self.manager.inhibit_shortcuts(surface, &self.seat);
        let connection = self.host.connection().clone();
        crate::wayland::inhibit::flush(&connection);
        ShortcutInhibitor::new(inhibitor, connection)
    }
}
