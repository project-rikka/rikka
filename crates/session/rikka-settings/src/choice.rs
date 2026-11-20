use rikka_types::Choice;

use crate::key::Key;

impl Key {
    #[must_use]
    pub fn choice<C: Choice>(name: impl Into<String>) -> Self {
        Key::string(name, C::default().slug()).choices(C::ALL.iter().map(|choice| choice.slug()))
    }
}
