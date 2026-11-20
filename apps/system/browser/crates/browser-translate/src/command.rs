use rikka_languages::Language;
use rikka_types::Choice;
use serde_json::{Value, json};

use crate::code_mode::CodeMode;
use crate::display::Display;
use crate::message::Watch;
use crate::script::TRANSLATOR_SCRIPT;

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
    pub fn answer(&self, frame: &Watch) -> Value {
        let mut answer = self.to_json();
        if matches!(self, Command::Translate { .. }) && !frame.translator {
            answer["translator"] = Value::from(TRANSLATOR_SCRIPT);
        }
        answer
    }

    fn to_json(&self) -> Value {
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
