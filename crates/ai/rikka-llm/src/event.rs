use crate::hosted_part::HostedPart;
use crate::response::Response;
use crate::tool_use::ToolUse;

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Event {
    Start {
        id: String,
        model: String,
    },
    Text(String),
    Reasoning(String),
    ToolStart {
        index: usize,
        id: String,
        name: String,
    },
    ToolInput {
        index: usize,
        delta: String,
    },
    ToolCall(ToolUse),
    Hosted(HostedPart),
    Done(Box<Response>),
}

impl Event {
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Event::Text(text) => Some(text),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_done(&self) -> Option<&Response> {
        match self {
            Event::Done(response) => Some(response),
            _ => None,
        }
    }
}
