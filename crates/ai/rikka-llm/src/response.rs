use serde::de::DeserializeOwned;

use crate::content::Content;
use crate::error::{Error, Result};
use crate::message::Message;
use crate::reasoning::Reasoning;
use crate::stop::Stop;
use crate::tool_use::ToolUse;
use rikka_tool::Usage;

#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    id: String,
    model: String,
    content: Vec<Content>,
    stop: Stop,
    usage: Usage,
}

impl Response {
    pub(crate) fn new(
        id: String,
        model: String,
        content: Vec<Content>,
        stop: Stop,
        usage: Usage,
    ) -> Response {
        Response {
            id,
            model,
            content,
            stop,
            usage,
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    #[must_use]
    pub fn content(&self) -> &[Content] {
        &self.content
    }

    #[must_use]
    pub fn stop(&self) -> &Stop {
        &self.stop
    }

    #[must_use]
    pub fn usage(&self) -> Usage {
        self.usage
    }

    #[must_use]
    pub fn text(&self) -> String {
        let mut text = String::new();
        for block in self.content.iter().filter_map(Content::as_text) {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(block);
        }
        text
    }

    pub fn tool_calls(&self) -> impl Iterator<Item = &ToolUse> {
        self.content.iter().filter_map(Content::as_tool_use)
    }

    pub fn reasoning(&self) -> impl Iterator<Item = &Reasoning> {
        self.content.iter().filter_map(Content::as_reasoning)
    }

    pub fn json<T: DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_str(&self.text()).map_err(|error| Error::Format(Box::new(error)))
    }

    #[must_use]
    pub fn message(&self) -> Message {
        Message::assistant(self.content.clone())
    }

    #[must_use]
    pub fn into_message(self) -> Message {
        Message::assistant(self.content)
    }
}
