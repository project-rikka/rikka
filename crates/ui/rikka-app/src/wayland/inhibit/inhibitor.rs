use rikka_wayland_client::Connection;
use rikka_wayland_client::zwp_keyboard_shortcuts_inhibitor_v1::ZwpKeyboardShortcutsInhibitorV1;

#[must_use]
pub(crate) struct ShortcutInhibitor {
    inhibitor: ZwpKeyboardShortcutsInhibitorV1,
    conn: Connection,
}

impl ShortcutInhibitor {
    pub(crate) fn new(
        inhibitor: ZwpKeyboardShortcutsInhibitorV1,
        conn: Connection,
    ) -> ShortcutInhibitor {
        ShortcutInhibitor { inhibitor, conn }
    }
}

impl Drop for ShortcutInhibitor {
    fn drop(&mut self) {
        self.inhibitor.destroy();
        crate::wayland::inhibit::flush(&self.conn);
    }
}

impl std::fmt::Debug for ShortcutInhibitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShortcutInhibitor").finish_non_exhaustive()
    }
}
