mod caption;
mod decor;
mod glass;
mod windows;

use rikka_compositor_planes::Tearing;
use rikka_ime::Keymap;
use rikka_session::profile::Profile;
use rikka_theme::{Palette, Theme};

pub(crate) use caption::CaptionLook;
pub(crate) use decor::Decor;
pub(crate) use glass::Glass;
pub(crate) use windows::{LayoutMode, SnapRules, WindowRules};

use crate::settings::SessionSettings;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Policy {
    windows: WindowRules,
    animate: bool,
    glass: Glass,
    minimizes_windows: bool,
    keymap: Option<Keymap>,
    palette: Palette,
    caption: CaptionLook,
    tearing: Tearing,
}

impl Policy {
    pub(crate) fn declared(profile: Profile) -> Self {
        let theme = Theme::default();
        Self {
            windows: WindowRules::declared().for_profile(profile),
            animate: true,
            glass: Glass::none(),
            minimizes_windows: true,
            keymap: None,
            palette: theme.palette(),
            caption: CaptionLook::of(&theme),
            tearing: Tearing::Allowed,
        }
    }

    pub(crate) fn read(settings: &mut SessionSettings) -> Self {
        Self {
            windows: settings.window_rules(),
            animate: settings.animations_enabled(),
            glass: settings.glass(),
            minimizes_windows: settings.minimizes_windows(),
            keymap: Some(settings.keymap()),
            palette: settings.palette(),
            caption: settings.caption(),
            tearing: settings.tearing(),
        }
    }

    pub(crate) fn window_rules(&self) -> WindowRules {
        self.windows
    }

    pub(crate) fn animates(&self) -> bool {
        self.animate
    }

    pub(crate) fn palette(&self) -> Palette {
        self.palette
    }

    pub(crate) fn caption(&self) -> CaptionLook {
        self.caption
    }

    pub(crate) fn minimizes_windows(&self) -> bool {
        self.minimizes_windows
    }

    pub(crate) fn keymap(&self) -> Option<&Keymap> {
        self.keymap.as_ref()
    }

    pub(crate) fn tearing(&self) -> Tearing {
        self.tearing
    }

    pub(crate) fn decor(&self) -> Decor {
        Decor {
            glass: self.glass,
            edge: self.palette.outline,
        }
    }

    pub(crate) fn follow(&mut self, next: Self) {
        if next == *self {
            return;
        }
        if next.animate != self.animate {
            let state = if next.animate { "on" } else { "off" };
            tracing::debug!("window animations: {state}");
        }
        if next.windows != self.windows {
            tracing::debug!("window layout mode: {:?}", next.windows.mode);
        }
        if next.glass != self.glass {
            tracing::debug!("translucency backdrop: {:?}", next.glass);
        }
        if next.minimizes_windows != self.minimizes_windows {
            tracing::debug!(
                "taskbar {} minimized windows",
                if next.minimizes_windows {
                    "takes"
                } else {
                    "refuses"
                }
            );
        }
        if next.tearing != self.tearing {
            tracing::debug!("fullscreen tearing: {:?}", next.tearing);
        }
        if next.keymap != self.keymap
            && let Some(keymap) = &next.keymap
        {
            tracing::debug!("keymap set to `{}`", keymap.describe());
        }
        *self = next;
    }
}
