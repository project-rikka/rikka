use rikka_languages::Language;
use serde_json::{Value, json};

use crate::unit::Output;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Translation {
    pub outputs: Vec<Output>,
    pub detected: Option<Language>,
}

impl Translation {
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({ "units": self.outputs.iter().map(Output::to_json).collect::<Vec<_>>() })
    }
}
