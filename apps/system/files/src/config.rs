use rikka_settings::seam::Preferences;
use rikka_settings::{Key, Schema};

const DOMAIN: &str = "rikka.files";
const AUTOMOUNT: &str = "automount";

#[derive(Debug)]
pub(crate) struct Config(Preferences);

impl Config {
    pub(crate) fn load() -> Self {
        let schema = Schema::new(DOMAIN)
            .summary("Files")
            .key(Key::bool(AUTOMOUNT, true).summary("Mount disks automatically when plugged in"));
        Self(Preferences::open(schema))
    }

    pub(crate) fn automount(&mut self) -> bool {
        self.0.refresh();
        self.0.get(AUTOMOUNT)
    }
}
