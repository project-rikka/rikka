use serde_json::Value;

use crate::assembler::Assemble;
use crate::cloud_code::decode;
use crate::error::{Error, Result};
use crate::event::Event;
use crate::response::Response;
use crate::transport::text_of;
use rikka_tool::Usage;

#[derive(Debug, Default)]
pub(crate) struct Assembler {
    started: bool,
    parts: Vec<Value>,
    tools: usize,
    id: String,
    model: String,
    stopped: Option<String>,
    usage: Usage,
}

impl Assemble for Assembler {
    fn feed(&mut self, data: &str) -> Result<Vec<Event>> {
        let data: Value =
            serde_json::from_str(data).map_err(|error| Error::Malformed(error.to_string()))?;
        let answer = decode::answer(&data);

        if let Some(error) = answer.get("error") {
            return Err(Error::Rejected(text_of(error, "message")));
        }

        let mut events = Vec::new();
        if !self.started {
            self.started = true;
            self.id = text_of(answer, "responseId");
            self.model = text_of(answer, "modelVersion");
            events.push(Event::Start {
                id: self.id.clone(),
                model: self.model.clone(),
            });
        }
        if let Some(usage) = answer.get("usageMetadata") {
            self.usage = decode::usage(Some(usage));
        }

        let Some(candidate) = decode::candidate(answer) else {
            return Ok(events);
        };
        if let Some(reason) = reason(candidate) {
            self.stopped = Some(reason.to_owned());
        }

        for part in decode::parts(candidate) {
            events.extend(self.absorb(part));
        }
        Ok(events)
    }

    fn finish(&mut self) -> Option<Event> {
        let reason = self.stopped.take()?;
        let mut content = Vec::new();
        decode::collect(&self.parts, &mut content);

        let stop = decode::stop(&reason, &content);
        Some(Event::Done(Box::new(Response::new(
            std::mem::take(&mut self.id),
            std::mem::take(&mut self.model),
            content,
            stop,
            self.usage,
        ))))
    }
}

impl Assembler {
    fn absorb(&mut self, part: &Value) -> Vec<Event> {
        let thought = part.get("thought").and_then(Value::as_bool) == Some(true);
        let text = text_of(part, "text");

        let events = match decode::tool_use(part, self.tools) {
            Some(call) => {
                let index = self.tools;
                self.tools += 1;

                vec![
                    Event::ToolStart {
                        index,
                        id: call.id().to_owned(),
                        name: call.name().to_owned(),
                    },
                    Event::ToolInput {
                        index,
                        delta: call.input().to_string(),
                    },
                    Event::ToolCall(call),
                ]
            }
            None if text.is_empty() => Vec::new(),
            None if thought => vec![Event::Reasoning(text.clone())],
            None => vec![Event::Text(text.clone())],
        };

        self.keep(part, thought, &text);
        events
    }

    fn keep(&mut self, part: &Value, thought: bool, text: &str) {
        if plain(part)
            && !text.is_empty()
            && let Some(last) = self.parts.last_mut()
        {
            let mergeable = plain(last)
                && (last.get("thought").and_then(Value::as_bool) == Some(true)) == thought;

            if mergeable && let Some(Value::String(carried)) = last.get_mut("text") {
                carried.push_str(text);
                return;
            }
        }
        self.parts.push(part.clone());
    }
}

fn plain(part: &Value) -> bool {
    part.get("functionCall").is_none() && decode::signature(part).is_none()
}

fn reason(candidate: &Value) -> Option<&str> {
    candidate
        .get("finishReason")
        .and_then(Value::as_str)
        .filter(|reason| !reason.is_empty())
}
