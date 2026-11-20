use rikka_settings::{Key, Schema};
use serde::{Deserialize, Serialize};

use crate::preferences::Preferences;

const ENGINE: &str = "search-engine";
const SUGGESTIONS: &str = "search-suggestions";
const CUSTOM: &str = "search-engines";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CustomEngine {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) keyword: String,
    pub(crate) url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) suggestions: String,
}

impl Preferences {
    pub(crate) fn search_engine(&self) -> String {
        self.observers.read();
        self.settings.borrow().get(ENGINE)
    }

    pub(crate) fn set_search_engine(&self, id: &str) {
        self.put::<String>(ENGINE, id.to_owned());
    }

    pub(crate) fn search_suggestions(&self) -> bool {
        self.observers.read();
        self.settings.borrow().get(SUGGESTIONS)
    }

    pub(crate) fn set_search_suggestions(&self, on: bool) {
        self.put::<bool>(SUGGESTIONS, on);
    }

    pub(crate) fn custom_engines(&self) -> Vec<CustomEngine> {
        self.observers.read();
        self.settings.borrow().get(CUSTOM)
    }

    pub(crate) fn set_custom_engines(&self, engines: &[CustomEngine]) {
        self.put::<Vec<CustomEngine>>(CUSTOM, engines.to_vec());
    }
}

pub(super) fn keys(schema: Schema) -> Schema {
    schema
        .key(Key::string(ENGINE, "google").summary("Default search engine"))
        .key(Key::bool(SUGGESTIONS, true).summary("Show search suggestions while typing"))
        .key(
            Key::list(CUSTOM, [])
                .summary("Custom search engines: id, name, keyword, url and suggestions"),
        )
}
