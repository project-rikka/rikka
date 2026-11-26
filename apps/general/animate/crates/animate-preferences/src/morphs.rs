use rikka_settings::{Key, Role, Schema};

use super::Preferences;

const OPEN: &str = "morphs-open";

impl Preferences {
    pub fn morphs_open(&self) -> bool {
        self.0.borrow().get::<bool>(OPEN)
    }

    pub fn keep_morphs_open(&self, open: bool) {
        self.0.borrow_mut().set(OPEN, open);
    }
}

pub(super) fn keys(schema: Schema) -> Schema {
    schema.key(
        Key::bool(OPEN, true)
            .summary("Show the expressions and morphs in the Motion tab")
            .role(Role::State),
    )
}
