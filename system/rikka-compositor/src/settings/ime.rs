use rikka_ime::{InputSource, Keymap};
use rikka_settings::Schema;
use rikka_settings::seam::Settings;
use rikka_settings::session::ime;

pub(crate) use rikka_settings::session::ime::DOMAIN;

pub(crate) fn schema() -> Schema {
    ime::schema()
}

pub(crate) fn keymap(settings: &Settings) -> Keymap {
    Keymap::from_sources(
        &sources(settings),
        settings
            .read::<String>(ime::MODEL)
            .unwrap_or_else(|| ime::MODEL_DEFAULT.to_owned()),
        settings.read::<String>(ime::OPTIONS),
    )
}

fn sources(settings: &Settings) -> Vec<InputSource> {
    settings.read(ime::SOURCES).unwrap_or_default()
}
