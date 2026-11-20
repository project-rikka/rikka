use rikka_llm::{Content, Message, Output, Request};

const BYTES_PER_TOKEN: u64 = 4;

const PICTURE: u64 = 2_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Gauge {
    counted: u64,
    at: usize,
}

impl Gauge {
    pub(crate) fn counted(counted: u64, at: usize) -> Gauge {
        Gauge { counted, at }
    }

    pub(crate) fn reading(&self, request: &Request) -> u64 {
        let messages = request.messages();
        match self.counted {
            0 => padded(guessed(messages) + framing(request)),
            counted => counted + padded(guessed(&messages[self.at.min(messages.len())..])),
        }
    }
}

fn padded(tokens: u64) -> u64 {
    tokens.saturating_mul(4).div_ceil(3)
}

fn guessed(messages: &[Message]) -> u64 {
    messages
        .iter()
        .flat_map(Message::content)
        .map(|content| match content {
            Content::Text(text) => estimate_tokens(text.as_str()),
            Content::Reasoning(reasoning) => estimate_tokens(reasoning.text()),
            Content::ToolUse(call) => {
                estimate_tokens(call.name()) + estimate_tokens(&call.input().to_string())
            }
            Content::ToolResult(result) => result
                .output()
                .iter()
                .map(|output| match output {
                    Output::Text(text) | Output::Reference(text) => estimate_tokens(text),
                    Output::Image(_) => PICTURE,
                })
                .sum(),
            Content::Image(_) | Content::Document(_) => PICTURE,
            Content::Hosted(hosted) => (hosted.bytes() as u64).div_ceil(BYTES_PER_TOKEN),
            _ => 0,
        })
        .sum()
}

fn framing(request: &Request) -> u64 {
    let system: u64 = request
        .system()
        .iter()
        .map(|text| estimate_tokens(text.as_str()))
        .sum();
    let tools: u64 = request
        .tools()
        .iter()
        .filter(|tool| !tool.is_deferred())
        .map(|tool| {
            estimate_tokens(tool.name())
                + estimate_tokens(tool.description())
                + estimate_tokens(&tool.schema().to_string())
        })
        .sum();
    system + tools
}

fn estimate_tokens(text: &str) -> u64 {
    (text.len() as u64).div_ceil(BYTES_PER_TOKEN)
}
