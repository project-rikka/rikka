use rikka_settings::{Key, Schema};

use crate::Preferences;

const CHECK_DEFAULT: &str = "check-default";

impl Preferences {
    pub fn check_default(&self) -> bool {
        self.observers.read();
        self.settings.borrow().get(CHECK_DEFAULT)
    }

    pub fn set_check_default(&self, check: bool) {
        self.put::<bool>(CHECK_DEFAULT, check);
    }
}

pub(super) fn keys(schema: Schema) -> Schema {
    schema.key(
        Key::bool(CHECK_DEFAULT, true)
            .summary("Offer to make Browser the default browser when it starts"),
    )
}
