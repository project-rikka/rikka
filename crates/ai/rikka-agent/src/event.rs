use rikka_llm::{HostedPart, Image, Message, Usage, Value};

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Event {
    Text(String),
    Reasoning(String),
    Responded(Message),
    Hosted(HostedPart),
    ToolStarted {
        id: String,
        name: String,
        input: Value,
    },
    ToolFinished {
        id: String,
        name: String,
        output: String,
        failed: bool,
        detail: Option<Value>,
        images: Vec<Image>,
        displayed: Vec<Image>,
        loaded: Vec<String>,
    },
    TurnFinished {
        turn: u32,
        usage: Usage,
        total_usage: Usage,
    },
    Resumed,
    Compacting,
    Compacted {
        summary: String,
        compacted_messages: usize,
    },
    CompactFailed {
        reason: String,
    },
    Noticed {
        text: String,
    },
    Interjected {
        id: u64,
    },
}

pub trait Observer: Send + Sync + 'static {
    fn observe(&self, event: Event);
}

impl<F: Fn(Event) + Send + Sync + 'static> Observer for F {
    fn observe(&self, event: Event) {
        self(event);
    }
}
