use crate::key::Key;
use crate::schema::Schema;

pub const DOMAIN: &str = "rikka.animation";

pub const ENABLED: &str = "enabled";

pub const ENABLED_DEFAULT: bool = true;

#[must_use]
pub fn schema() -> Schema {
    Schema::new(DOMAIN)
        .summary("Window animations")
        .key(Key::bool(ENABLED, ENABLED_DEFAULT).summary("Animate window open, close and resize"))
}
