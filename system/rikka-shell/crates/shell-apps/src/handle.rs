use rikka_image::ColorScheme;
use rikka_settings::session::appearance::ICON_THEME_DEFAULT;
use rikka_theme::Theme;
use shell_kit::{Shell, appearance};

use crate::{AppIcons, Installed};

#[derive(Clone, Debug)]
pub struct Apps {
    installed: Installed,
    icons: AppIcons,
}

impl Apps {
    #[must_use]
    pub fn follow(shell: Shell) -> (Shell, Apps) {
        let installed = Installed::watched();
        let shared_theme = shell.shared_theme();
        let icons = AppIcons::read(
            ICON_THEME_DEFAULT,
            installed.clone(),
            scheme(&shared_theme.theme()),
        );
        shared_theme.on_change({
            let icons = icons.clone();
            move |theme| icons.follow_scheme(scheme(theme))
        });
        let shell = shell.follow(appearance::DOMAIN, appearance::ICON_THEME, {
            let icons = icons.clone();
            move |_, name: String| icons.wear(appearance::icon_theme(name))
        });
        (shell, Apps { installed, icons })
    }

    #[must_use]
    pub fn installed(&self) -> &Installed {
        &self.installed
    }

    #[must_use]
    pub fn icons(&self) -> &AppIcons {
        &self.icons
    }
}

fn scheme(theme: &Theme) -> ColorScheme {
    match theme.is_dark() {
        true => ColorScheme::Dark,
        false => ColorScheme::Light,
    }
}
