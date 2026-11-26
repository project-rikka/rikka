use rikka_settings::{Key, Schema};

use crate::Preferences;

const HTTPS_ONLY: &str = "https-only";

impl Preferences {
    pub fn https_only(&self) -> bool {
        self.observers.read();
        self.settings.borrow().get(HTTPS_ONLY)
    }

    pub fn set_https_only(&self, on: bool) {
        self.put::<bool>(HTTPS_ONLY, on);
    }
}

pub(super) fn keys(schema: Schema) -> Schema {
    schema.key(Key::bool(HTTPS_ONLY, true).summary("Upgrade every connection to HTTPS"))
}
