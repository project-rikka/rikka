use rikka_wayland::Host;
use rikka_wayland_client::rikka_window_view_manager_v1::RikkaWindowViewManagerV1;
use rikka_wayland_client::wl_surface::WlSurface;

use crate::shown::Shown;

#[must_use]
#[derive(Debug)]
pub struct WindowViews {
    manager: RikkaWindowViewManagerV1,
}

impl WindowViews {
    pub fn bind(host: &Host) -> Option<WindowViews> {
        let manager = host.display().bind_optional(1..=1)?;
        Some(WindowViews { manager })
    }

    pub fn inside(&self, surface: &WlSurface) -> Shown {
        Shown::new(self.manager.clone(), surface.clone())
    }
}

impl Drop for WindowViews {
    fn drop(&mut self) {
        self.manager.destroy();
    }
}
