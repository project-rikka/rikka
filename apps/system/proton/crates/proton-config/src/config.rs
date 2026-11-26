use rikka_settings::seam::Preferences;
use rikka_settings::{Key, Schema};

const DOMAIN: &str = "rikka.proton";
const CONVERSATIONS: &str = "conversations";
const DRIVE_TILES: &str = "drive-tiles";
const STAY_SIGNED_IN: &str = "stay-signed-in";

#[derive(Debug)]
pub struct Config(Preferences);

impl Config {
    #[must_use]
    pub fn load() -> Config {
        Config(Preferences::open(Self::schema()))
    }

    #[must_use]
    pub fn schema() -> Schema {
        Schema::new(DOMAIN)
            .summary("Proton")
            .key(Key::bool(CONVERSATIONS, true).summary("Group mail by conversation"))
            .key(Key::bool(DRIVE_TILES, true).summary("Show Drive as tiles"))
            .key(
                Key::bool(STAY_SIGNED_IN, true)
                    .summary("Keep the session and mailbox key on this device"),
            )
    }

    #[must_use]
    pub fn conversations(&self) -> bool {
        self.0.get(CONVERSATIONS)
    }

    pub fn set_conversations(&mut self, grouped: bool) {
        self.0.set(CONVERSATIONS, grouped);
    }

    #[must_use]
    pub fn drive_tiles(&self) -> bool {
        self.0.get(DRIVE_TILES)
    }

    pub fn set_drive_tiles(&mut self, tiles: bool) {
        self.0.set(DRIVE_TILES, tiles);
    }

    #[must_use]
    pub fn stay_signed_in(&self) -> bool {
        self.0.get(STAY_SIGNED_IN)
    }

    pub fn set_stay_signed_in(&mut self, stay: bool) {
        self.0.set(STAY_SIGNED_IN, stay);
    }
}
