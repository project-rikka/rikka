use jiff::Timestamp;
use rikka_proton_api::Outcome;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct UndoToken {
    #[serde(rename = "Token", default)]
    token: String,
    #[serde(
        rename = "ValidUntil",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    valid_until: Timestamp,
}

impl UndoToken {
    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }

    #[must_use]
    pub fn valid_until(&self) -> Timestamp {
        self.valid_until
    }
}

#[derive(Debug, Clone, Default)]
pub struct BatchOutcome {
    items: Vec<Outcome<String>>,
    undo: Vec<UndoToken>,
}

impl BatchOutcome {
    #[must_use]
    pub fn items(&self) -> &[Outcome<String>] {
        &self.items
    }

    #[must_use]
    pub fn undo_tokens(&self) -> &[UndoToken] {
        &self.undo
    }

    pub fn failures(&self) -> impl Iterator<Item = &Outcome<String>> {
        self.items.iter().filter(|item| !item.is_ok())
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.items.iter().all(Outcome::is_ok)
    }

    pub(crate) fn merge(&mut self, items: Vec<Outcome<String>>, undo: Option<UndoToken>) {
        self.items.extend(items);
        self.undo.extend(undo);
    }
}
