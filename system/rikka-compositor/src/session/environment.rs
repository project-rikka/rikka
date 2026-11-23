use std::ffi::OsString;
use std::process::Command;

use rikka_gpu_drivers::VULKAN_SELECT_ENV;
use rikka_session::environment::BUS_ADDRESS_ENV;
use rikka_session::profile::PROFILE_ENV;

use crate::context::SessionRef;
use crate::programs;
use crate::settings::SessionSettings;

pub(crate) const DESKTOP_NAME: &str = "rikka";

impl SessionRef<'_> {
    pub(crate) fn session_command(&self, program: &str) -> Command {
        let mut command = Command::new(program);
        for (name, value) in self.session_environment() {
            match value {
                Some(value) => command.env(name, value),
                None => command.env_remove(name),
            };
        }
        command
    }

    pub(crate) fn session_environment(&self) -> Vec<(&'static str, Option<OsString>)> {
        let (cursor_theme, cursor_size) = self.cursor.theme_and_size();
        let x_display = self.x11.display_number();
        let mut environment = vec![
            ("WAYLAND_DISPLAY", Some(self.core.socket_name().into())),
            ("XCURSOR_THEME", Some(cursor_theme.into())),
            ("XCURSOR_SIZE", Some(cursor_size.to_string().into())),
            ("XDG_CURRENT_DESKTOP", Some(DESKTOP_NAME.into())),
            ("XDG_SESSION_DESKTOP", Some(DESKTOP_NAME.into())),
            ("XDG_SESSION_TYPE", Some("wayland".into())),
            (PROFILE_ENV, Some(self.core.profile.name().into())),
            ("GTK_USE_PORTAL", Some("1".into())),
            ("QT_QPA_PLATFORMTHEME", Some("xdgdesktopportal".into())),
            ("ELECTRON_OZONE_PLATFORM_HINT", Some("auto".into())),
            ("MOZ_ENABLE_WAYLAND", Some("1".into())),
            (
                rikka_seam::net::SOCKET_ENV,
                self.session
                    .seam
                    .socket()
                    .map(|socket| socket.as_os_str().to_os_string()),
            ),
            (
                "DISPLAY",
                x_display.map(|display| format!(":{display}").into()),
            ),
            (
                "XMODIFIERS",
                x_display.map(|_| format!("@im={}", rikka_ime::IM_NAME).into()),
            ),
        ];
        if let Some(path) = programs::search_path() {
            environment.push(("PATH", Some(path)));
        }
        if let Some(selected) = std::env::var_os(VULKAN_SELECT_ENV) {
            environment.push((VULKAN_SELECT_ENV, Some(selected)));
        }
        if let Some(bus) = self.session.standing.private_bus() {
            environment.push((BUS_ADDRESS_ENV, Some(bus.address().into())));
        }
        if let Some(settings) = &self.settings {
            environment.extend(SessionSettings::proxy_environment(settings));
        }
        environment
    }
}
