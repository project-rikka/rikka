use rikka_languages::Language;
use rikka_types::Choice;
use serde_json::{Value, json};

use crate::code_mode::CodeMode;
use crate::display::Display;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Translate {
        session: u64,
        to: Language,
        display: Display,
        code: CodeMode,
    },
    Restore,
}

impl Command {
    #[must_use]
    pub fn answer(&self) -> Value {
        match self {
            Command::Translate {
                session,
                to,
                display,
                code,
            } => json!({
                "do": "translate",
                "session": session,
                "to": to.tag(),
                "display": display.slug(),
                "code": code.slug(),
            }),
            Command::Restore => json!({ "do": "restore" }),
        }
    }
}
