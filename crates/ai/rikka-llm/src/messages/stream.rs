use serde_json::{Value, json};

use crate::assembler::Assemble;
use crate::content::Content;
use crate::error::{Error, Result};
use crate::event::Event;
use crate::hosted::Hosted;
use crate::messages::{decode, hosted};
use crate::reasoning::Reasoning;
use crate::response::Response;
use crate::stop::Stop;
use crate::text::Text;
use crate::tool_use::ToolUse;
use crate::transport::{arguments, text_of, unless_cut_short};
use crate::vendor::Vendor;
use rikka_tool::Usage;

const MAX_BLOCKS: usize = 1024;

#[derive(Debug)]
enum Block {
    Text(String),
    Reasoning {
        text: String,
        signature: String,
    },
    Redacted(Value),
    Tool {
        ordinal: usize,
        id: String,
        name: String,
        input: String,
    },
    ServerCall {
        block: Value,
        input: String,
    },
    ServerResult(Value),
    Ignored,
}

#[derive(Debug, Default)]
pub(crate) struct Assembler {
    id: String,
    model: String,
    blocks: Vec<Block>,
    content: Vec<Content>,
    calls: usize,
    unreadable: Option<Error>,
    stop: Stop,
    usage: Usage,
}

impl Assemble for Assembler {
    fn feed(&mut self, data: &str) -> Result<Vec<Event>> {
        let data: Value =
            serde_json::from_str(data).map_err(|error| Error::Malformed(error.to_string()))?;

        match text_of(&data, "type").as_str() {
            "message_start" => Ok(self.start(&data)),
            "content_block_start" => Ok(self.open(&data)),
            "content_block_delta" => Ok(self.delta(&data)),
            "content_block_stop" => self.close(&data),
            "message_delta" => {
                self.message_delta(&data);
                Ok(Vec::new())
            }
            "message_stop" => Ok(vec![self.finished()?]),
            "error" => Err(decode::stream_error(&data)),
            _ => Ok(Vec::new()),
        }
    }

    fn finish(&mut self) -> Option<Event> {
        None
    }
}

impl Assembler {
    fn start(&mut self, data: &Value) -> Vec<Event> {
        let Some(message) = data.get("message") else {
            return Vec::new();
        };

        self.id = text_of(message, "id");
        self.model = text_of(message, "model");
        self.usage = decode::usage(message.get("usage"));

        vec![Event::Start {
            id: self.id.clone(),
            model: self.model.clone(),
        }]
    }

    fn open(&mut self, data: &Value) -> Vec<Event> {
        let Some(block) = data.get("content_block") else {
            return Vec::new();
        };

        let Some(index) = index_of(data) else {
            return Vec::new();
        };
        if !matches!(self.blocks.get(index), None | Some(Block::Ignored)) {
            return Vec::new();
        }
        while self.blocks.len() <= index {
            self.blocks.push(Block::Ignored);
        }

        let (started, events) = match text_of(block, "type").as_str() {
            "text" => (Block::Text(text_of(block, "text")), Vec::new()),
            "thinking" => (
                Block::Reasoning {
                    text: text_of(block, "thinking"),
                    signature: String::new(),
                },
                Vec::new(),
            ),
            "redacted_thinking" => (Block::Redacted(block.clone()), Vec::new()),
            "tool_use" => {
                let id = text_of(block, "id");
                let name = text_of(block, "name");
                let ordinal = self.calls;
                self.calls += 1;
                (
                    Block::Tool {
                        ordinal,
                        id: id.clone(),
                        name: name.clone(),
                        input: String::new(),
                    },
                    vec![Event::ToolStart {
                        index: ordinal,
                        id,
                        name,
                    }],
                )
            }
            "server_tool_use" => (
                Block::ServerCall {
                    block: block.clone(),
                    input: String::new(),
                },
                Vec::new(),
            ),
            _ if hosted::is_block(block) => (Block::ServerResult(block.clone()), Vec::new()),
            _ => (Block::Ignored, Vec::new()),
        };

        self.blocks[index] = started;
        events
    }

    fn delta(&mut self, data: &Value) -> Vec<Event> {
        let Some(index) = index_of(data) else {
            return Vec::new();
        };
        let Some(delta) = data.get("delta") else {
            return Vec::new();
        };
        let Some(block) = self.blocks.get_mut(index) else {
            return Vec::new();
        };

        match (text_of(delta, "type").as_str(), block) {
            ("text_delta", Block::Text(text)) => {
                let piece = text_of(delta, "text");
                text.push_str(&piece);
                vec![Event::Text(piece)]
            }
            ("thinking_delta", Block::Reasoning { text, .. }) => {
                let piece = text_of(delta, "thinking");
                text.push_str(&piece);
                vec![Event::Reasoning(piece)]
            }
            ("signature_delta", Block::Reasoning { signature, .. }) => {
                signature.push_str(&text_of(delta, "signature"));
                Vec::new()
            }
            ("input_json_delta", Block::Tool { ordinal, input, .. }) => {
                let piece = text_of(delta, "partial_json");
                let index = *ordinal;
                input.push_str(&piece);
                vec![Event::ToolInput {
                    index,
                    delta: piece,
                }]
            }
            ("input_json_delta", Block::ServerCall { input, .. }) => {
                input.push_str(&text_of(delta, "partial_json"));
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn close(&mut self, data: &Value) -> Result<Vec<Event>> {
        let Some(index) = index_of(data) else {
            return Ok(Vec::new());
        };
        let Some(block) = self.blocks.get(index) else {
            return Ok(Vec::new());
        };

        let content = match block {
            Block::Text(text) => Content::Text(Text::new(text.clone())),
            Block::Reasoning { text, signature } => {
                let payload = json!({
                    "type": "thinking",
                    "thinking": text,
                    "signature": signature,
                });
                Content::Reasoning(Reasoning::opaque(Vendor::Anthropic, text.clone(), payload))
            }
            Block::Redacted(value) => Content::Reasoning(Reasoning::opaque(
                Vendor::Anthropic,
                String::new(),
                value.clone(),
            )),
            Block::Tool {
                id, name, input, ..
            } => match arguments(input) {
                Ok(input) => Content::ToolUse(ToolUse::new(id.clone(), name.clone(), input)),
                Err(error) => {
                    self.unreadable.get_or_insert(error);
                    self.blocks[index] = Block::Ignored;
                    return Ok(Vec::new());
                }
            },
            Block::ServerCall { block, input } => match arguments(input) {
                Ok(input) => {
                    let mut block = block.clone();
                    block["input"] = input;
                    match Hosted::read(Vendor::Anthropic, block) {
                        Some(hosted) => Content::Hosted(hosted),
                        None => return Ok(self.ignore(index)),
                    }
                }
                Err(error) => {
                    self.unreadable.get_or_insert(error);
                    return Ok(self.ignore(index));
                }
            },
            Block::ServerResult(block) => match Hosted::read(Vendor::Anthropic, block.clone()) {
                Some(hosted) => Content::Hosted(hosted),
                None => return Ok(self.ignore(index)),
            },
            Block::Ignored => return Ok(Vec::new()),
        };

        self.blocks[index] = Block::Ignored;
        let events = match &content {
            Content::ToolUse(call) => vec![Event::ToolCall(call.clone())],
            Content::Hosted(hosted) => vec![Event::Hosted(hosted.part().clone())],
            _ => Vec::new(),
        };
        self.content.push(content);
        Ok(events)
    }

    fn ignore(&mut self, index: usize) -> Vec<Event> {
        self.blocks[index] = Block::Ignored;
        Vec::new()
    }

    fn message_delta(&mut self, data: &Value) {
        if let Some(delta) = data.get("delta") {
            self.stop = decode::stop(delta);
        }
        let usage = decode::usage(data.get("usage"));
        self.usage.output = usage.output.max(self.usage.output);
        self.usage.reasoning = usage.reasoning.max(self.usage.reasoning);
        if usage.input > 0 {
            self.usage.input = usage.input;
        }
        if usage.cache_read > 0 {
            self.usage.cache_read = usage.cache_read;
        }
        if usage.cache_write > 0 {
            self.usage.cache_write = usage.cache_write;
        }
    }

    fn finished(&mut self) -> Result<Event> {
        if let Some(error) = self.unreadable.take() {
            unless_cut_short(Err::<(), Error>(error), &self.stop)?;
        }

        Ok(Event::Done(Box::new(Response::new(
            std::mem::take(&mut self.id),
            std::mem::take(&mut self.model),
            std::mem::take(&mut self.content),
            std::mem::take(&mut self.stop),
            self.usage,
        ))))
    }
}

fn index_of(data: &Value) -> Option<usize> {
    data.get("index")
        .and_then(Value::as_u64)
        .and_then(|index| usize::try_from(index).ok())
        .filter(|index| *index < MAX_BLOCKS)
}
