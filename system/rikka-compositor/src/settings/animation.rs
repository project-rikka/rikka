use rikka_settings::Schema;
use rikka_settings::seam::Settings;
use rikka_settings::session::animation::{self, ENABLED, ENABLED_DEFAULT};

pub(crate) use rikka_settings::session::animation::DOMAIN;

pub(crate) fn schema() -> Schema {
    animation::schema()
}

pub(crate) fn enabled(settings: &Settings) -> bool {
    settings.read_or(ENABLED, ENABLED_DEFAULT)
}
