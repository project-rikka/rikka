use rikka_app::prelude::*;
use rikka_settings::session::displays::AdaptiveSync;

pub(crate) fn label(adaptive_sync: AdaptiveSync) -> Label {
    match adaptive_sync {
        AdaptiveSync::Never => tr!("settings.displays.vrr.never"),
        AdaptiveSync::Automatic => tr!("settings.displays.vrr.automatic"),
        AdaptiveSync::Always => tr!("settings.displays.vrr.always"),
    }
}
