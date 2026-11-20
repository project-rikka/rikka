use std::collections::BTreeMap;

use crate::key::Key;
use crate::schema::Schema;
use crate::value::Value;

pub const DOMAIN: &str = "rikka.ime";

pub const SOURCES: &str = "sources";

pub const MODEL: &str = "keyboard-model";

pub const OPTIONS: &str = "keyboard-options";

pub const LAYOUT: &str = "layout";
pub const VARIANT: &str = "variant";
pub const ENGINE: &str = "engine";

pub const MODEL_DEFAULT: &str = "pc105";

pub const UNSET: &str = "";

#[must_use]
pub fn layout(name: &str) -> Value {
    Value::Table(BTreeMap::from([(
        LAYOUT.to_owned(),
        Value::String(name.to_owned()),
    )]))
}

#[must_use]
pub fn default_sources() -> Vec<Value> {
    vec![layout("us")]
}

#[must_use]
pub fn schema() -> Schema {
    Schema::new(DOMAIN)
        .summary("What the keyboard types, and what composes on top of it")
        .key(
            Key::list(SOURCES, default_sources())
                .summary("The keyboard layouts the seat types in, and their input methods"),
        )
        .key(Key::string(MODEL, MODEL_DEFAULT).summary("The xkb model the keyboard is"))
        .key(
            Key::string(OPTIONS, UNSET)
                .summary("xkb options, comma separated — `caps:escape` and the like"),
        )
}
