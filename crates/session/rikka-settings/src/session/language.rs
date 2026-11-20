use crate::key::Key;
use crate::schema::Schema;
use crate::store::Store;

pub const DOMAIN: &str = "rikka.language";

pub const LANGUAGE: &str = "language";

pub const UNSET: &str = "";

#[must_use]
pub fn schema() -> Schema {
    Schema::new(DOMAIN)
        .summary("What language the desktop speaks")
        .key(
            Key::string(LANGUAGE, UNSET)
                .summary("A BCP 47 language tag, or nothing to follow the system's own language"),
        )
}

#[must_use]
pub fn selected(store: &Store) -> Option<String> {
    store
        .get::<String>(LANGUAGE)
        .inspect_err(|error| tracing::debug!("failed to parse language setting: {error}"))
        .ok()
        .filter(|tag| tag != UNSET)
}
