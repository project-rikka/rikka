use rikka_llm::{Message, Role, Stop, Usage};
use serde::de::DeserializeOwned;

use crate::budget::Limit;
use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    text: String,
    messages: Vec<Message>,
    usage: Usage,
    turns: u32,
    stop: Stop,
    exhausted: Option<Limit>,
}

impl Outcome {
    pub(crate) fn new(
        messages: Vec<Message>,
        usage: Usage,
        turns: u32,
        stop: Stop,
        exhausted: Option<Limit>,
        carried: String,
    ) -> Outcome {
        let last = messages
            .iter()
            .rev()
            .find(|message| message.role() == Role::Assistant)
            .map(Message::text)
            .unwrap_or_default();

        Outcome {
            text: carried + &last,
            messages,
            usage,
            turns,
            stop,
            exhausted,
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn json<T: DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_str(&self.text)
            .map_err(|error| Error::Model(rikka_llm::Error::Format(Box::new(error))))
    }

    #[must_use]
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    #[must_use]
    pub fn usage(&self) -> Usage {
        self.usage
    }

    #[must_use]
    pub fn turns(&self) -> u32 {
        self.turns
    }

    #[must_use]
    pub fn stop(&self) -> &Stop {
        &self.stop
    }

    #[must_use]
    pub fn exhausted(&self) -> Option<Limit> {
        self.exhausted
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.exhausted.is_none() && self.stop.is_complete()
    }
}
