use serde_json::{Map, Value};

use crate::assembler::Assemble;
use crate::completions::CompletionsDialect;
use crate::completions::decode;
use crate::completions::thinking::{Split, Thinking};
use crate::content::Content;
use crate::error::{Error, Result};
use crate::event::Event;
use crate::failure;
use crate::reasoning::Reasoning;
use crate::response::Response;
use crate::stop::Stop;
use crate::text::Text;
use crate::tool_use::ToolUse;
use crate::transport::{arguments, text_of, unless_cut_short};
use rikka_tool::Usage;

const DONE: &str = "[DONE]";

const MAX_CALLS: usize = 1024;

const MAX_DETAILS: usize = 1024;

const GROWN: [&str; 3] = ["text", "summary", "data"];

#[derive(Debug, Default)]
struct Call {
    id: String,
    name: String,
    arguments: String,
    announced: bool,
}

#[derive(Debug)]
pub(crate) struct Assembler {
    dialect: CompletionsDialect,
    id: String,
    model: String,
    text: String,
    reasoning: String,
    thinking: Option<Thinking>,
    details: Vec<Value>,
    calls: Vec<Call>,
    stop: Stop,
    usage: Usage,
    started: bool,
    finished: bool,
}

impl Assembler {
    pub(crate) fn new(dialect: CompletionsDialect) -> Assembler {
        Assembler {
            dialect,
            id: String::new(),
            model: String::new(),
            text: String::new(),
            reasoning: String::new(),
            thinking: dialect.thinks_inline().then(Thinking::default),
            details: Vec::new(),
            calls: Vec::new(),
            stop: Stop::default(),
            usage: Usage::default(),
            started: false,
            finished: false,
        }
    }
}

impl Assemble for Assembler {
    fn feed(&mut self, data: &str) -> Result<Vec<Event>> {
        if data.trim() == DONE {
            if self.finished {
                return Ok(Vec::new());
            }
            let mut events = self.unthought();
            events.push(self.finished()?);
            return Ok(events);
        }

        let chunk: Value =
            serde_json::from_str(data).map_err(|error| Error::Malformed(error.to_string()))?;

        if let Some(error) = failure::streamed(&chunk) {
            return Err(error);
        }

        let mut events = self.start(&chunk);
        if let Some(usage) = chunk.get("usage").filter(|usage| !usage.is_null()) {
            self.usage = decode::usage(Some(usage));
        }

        let choice = chunk
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first());

        let Some(choice) = choice else {
            return Ok(events);
        };

        if !text_of(choice, "finish_reason").is_empty() {
            self.stop = decode::stop(Some(choice), None);
        }
        if let Some(delta) = choice.get("delta") {
            events.extend(self.delta(delta));
        }

        Ok(events)
    }

    fn finish(&mut self) -> Option<Event> {
        if self.finished || !self.started {
            return None;
        }
        self.finished().ok()
    }
}

impl Assembler {
    fn start(&mut self, chunk: &Value) -> Vec<Event> {
        if self.started {
            return Vec::new();
        }

        self.started = true;
        self.id = text_of(chunk, "id");
        self.model = text_of(chunk, "model");

        vec![Event::Start {
            id: self.id.clone(),
            model: self.model.clone(),
        }]
    }

    fn delta(&mut self, delta: &Value) -> Vec<Event> {
        let mut events = Vec::new();

        if let Some(text) = decode::reasoning_of(delta) {
            self.reasoning.push_str(&text);
            events.push(Event::Reasoning(text));
        }
        if self.dialect.replays() {
            self.detailed(delta);
        }

        let text = text_of(delta, "content");
        if !text.is_empty() {
            let split = match self.thinking.as_mut() {
                Some(thinking) => thinking.split(&text),
                None => Split {
                    reasoning: String::new(),
                    text,
                },
            };
            events.extend(self.spoken(split));
        }

        for call in delta
            .get("tool_calls")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            events.extend(self.call(call));
        }

        events
    }

    fn spoken(&mut self, split: Split) -> Vec<Event> {
        let mut events = Vec::new();
        if !split.reasoning.is_empty() {
            self.reasoning.push_str(&split.reasoning);
            events.push(Event::Reasoning(split.reasoning));
        }
        if !split.text.is_empty() {
            self.text.push_str(&split.text);
            events.push(Event::Text(split.text));
        }
        events
    }

    fn unthought(&mut self) -> Vec<Event> {
        match self.thinking.as_mut().map(Thinking::finish) {
            Some(split) => self.spoken(split),
            None => Vec::new(),
        }
    }

    fn detailed(&mut self, delta: &Value) {
        let details = delta
            .get("reasoning_details")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();

        for (nth, detail) in details.iter().enumerate() {
            let index = detail
                .get("index")
                .and_then(Value::as_u64)
                .and_then(|index| usize::try_from(index).ok())
                .unwrap_or(nth);
            if index >= MAX_DETAILS {
                continue;
            }

            while self.details.len() <= index {
                self.details.push(Value::Object(Map::new()));
            }
            merge(&mut self.details[index], detail);
        }
    }

    fn call(&mut self, call: &Value) -> Vec<Event> {
        let function = call.get("function").unwrap_or(&Value::Null);
        let id = text_of(call, "id");
        let name = text_of(function, "name");
        let known = (!id.is_empty())
            .then(|| self.calls.iter().position(|pending| pending.id == id))
            .flatten();
        let index = match (known, call.get("index")) {
            (Some(known), _) => Some(known),
            (None, Some(value)) => value
                .as_u64()
                .and_then(|index| usize::try_from(index).ok())
                .map(|index| self.renumbered(index, &id)),
            (None, None) => Some(self.unnumbered(&id, &name)),
        }
        .filter(|index| *index < MAX_CALLS);

        let Some(index) = index else {
            return Vec::new();
        };

        while self.calls.len() <= index {
            self.calls.push(Call::default());
        }

        let piece = decode::arguments_of(function);

        let mut events = Vec::new();
        let pending = &mut self.calls[index];

        if pending.id.is_empty() && !id.is_empty() {
            pending.id = id;
        }
        if !name.is_empty() {
            pending.name = name;
        }
        if !pending.announced && !pending.name.is_empty() {
            pending.announced = true;
            if pending.id.is_empty() {
                pending.id = decode::minted_call_id();
            }
            events.push(Event::ToolStart {
                index,
                id: pending.id.clone(),
                name: pending.name.clone(),
            });
        }
        if !piece.is_empty() {
            pending.arguments.push_str(&piece);
            events.push(Event::ToolInput {
                index,
                delta: piece,
            });
        }

        events
    }

    fn renumbered(&self, index: usize, id: &str) -> usize {
        match self.calls.get(index) {
            Some(taken) if !id.is_empty() && !taken.id.is_empty() && taken.id != id => {
                self.calls.len()
            }
            _ => index,
        }
    }

    fn unnumbered(&self, id: &str, name: &str) -> usize {
        let Some(last) = self.calls.last() else {
            return 0;
        };
        let other_id = !id.is_empty() && !last.id.is_empty() && last.id != id;
        let other_name = !name.is_empty() && !last.name.is_empty();
        match other_id || other_name {
            true => self.calls.len(),
            false => self.calls.len() - 1,
        }
    }

    fn finished(&mut self) -> Result<Event> {
        self.finished = true;
        let unthought = self
            .thinking
            .as_mut()
            .map(Thinking::finish)
            .unwrap_or_default();
        self.reasoning.push_str(&unthought.reasoning);
        self.text.push_str(&unthought.text);

        let mut content = Vec::new();
        let reasoning = std::mem::take(&mut self.reasoning);
        let details: Vec<Value> = std::mem::take(&mut self.details)
            .into_iter()
            .filter(|detail| detail.as_object().is_some_and(|block| !block.is_empty()))
            .collect();

        if details.is_empty() {
            if !reasoning.is_empty() {
                content.push(Content::Reasoning(Reasoning::new(reasoning)));
            }
        } else {
            content.push(Content::Reasoning(Reasoning::opaque(
                self.dialect.vendor(),
                reasoning,
                Value::Array(details),
            )));
        }
        if !self.text.is_empty() {
            content.push(Content::Text(Text::new(std::mem::take(&mut self.text))));
        }
        for call in std::mem::take(&mut self.calls) {
            let id = match call.id.is_empty() {
                true => decode::minted_call_id(),
                false => call.id,
            };
            let called = arguments(&call.arguments)
                .map(|input| Content::ToolUse(ToolUse::new(id, call.name, input)));
            content.extend(unless_cut_short(called, &self.stop)?);
        }
        if self.stop == Stop::End
            && content
                .iter()
                .any(|part| matches!(part, Content::ToolUse(_)))
        {
            self.stop = Stop::ToolUse;
        }

        Ok(Event::Done(Box::new(Response::new(
            std::mem::take(&mut self.id),
            std::mem::take(&mut self.model),
            content,
            std::mem::take(&mut self.stop),
            self.usage,
        ))))
    }
}

fn merge(gathered: &mut Value, piece: &Value) {
    let (Some(gathered), Some(piece)) = (gathered.as_object_mut(), piece.as_object()) else {
        return;
    };

    for (field, value) in piece {
        if value.is_null() {
            continue;
        }
        match (gathered.get_mut(field), value) {
            (Some(Value::String(grown)), Value::String(added))
                if GROWN.contains(&field.as_str()) =>
            {
                grown.push_str(added);
            }
            _ => {
                gathered.insert(field.clone(), value.clone());
            }
        }
    }
}
