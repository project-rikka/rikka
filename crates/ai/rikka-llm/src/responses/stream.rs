use serde_json::{Value, json};

use crate::assembler::Assemble;
use crate::error::{Error, Result};
use crate::event::Event;
use crate::responses::decode;
use crate::transport::text_of;
use crate::vendor::Vendor;

const MAX_ITEMS: usize = 1024;

#[derive(Debug)]
pub(crate) struct Assembler {
    id: String,
    model: String,
    vendor: Vendor,
    tools: Vec<OpenTool>,
    output: Vec<Value>,
}

#[derive(Debug)]
struct OpenTool {
    item: String,
    at: Option<u64>,
}

impl Assembler {
    pub(crate) fn new(vendor: Vendor) -> Assembler {
        Assembler {
            id: String::new(),
            model: String::new(),
            vendor,
            tools: Vec::new(),
            output: Vec::new(),
        }
    }
}

impl Assemble for Assembler {
    fn feed(&mut self, data: &str) -> Result<Vec<Event>> {
        let data: Value =
            serde_json::from_str(data).map_err(|error| Error::Malformed(error.to_string()))?;

        match text_of(&data, "type").as_str() {
            "response.created" => Ok(self.created(&data)),
            "response.output_item.added" => Ok(self.added(&data)),
            "response.output_item.done" => Ok(self.item_done(&data)),
            "response.output_text.delta" => Ok(vec![Event::Text(text_of(&data, "delta"))]),
            "response.reasoning_summary_text.delta" | "response.reasoning_text.delta" => {
                Ok(vec![Event::Reasoning(text_of(&data, "delta"))])
            }
            "response.function_call_arguments.delta" => Ok(self.input(&data)),
            "response.completed" | "response.incomplete" => self.completed(&data),
            "response.failed" => Err(failed(&data)),
            "error" => Err(Error::Rejected(text_of(&data, "message"))),
            _ => Ok(Vec::new()),
        }
    }

    fn finish(&mut self) -> Option<Event> {
        None
    }
}

impl Assembler {
    fn created(&mut self, data: &Value) -> Vec<Event> {
        let Some(response) = data.get("response") else {
            return Vec::new();
        };
        self.id = text_of(response, "id");
        self.model = text_of(response, "model");

        vec![Event::Start {
            id: self.id.clone(),
            model: self.model.clone(),
        }]
    }

    fn added(&mut self, data: &Value) -> Vec<Event> {
        let Some(item) = data.get("item") else {
            return Vec::new();
        };
        if text_of(item, "type") != "function_call" {
            return Vec::new();
        }

        let index = self.tools.len();
        self.tools.push(OpenTool {
            item: text_of(item, "id"),
            at: output_index(data),
        });

        vec![Event::ToolStart {
            index,
            id: text_of(item, "call_id"),
            name: text_of(item, "name"),
        }]
    }

    fn input(&mut self, data: &Value) -> Vec<Event> {
        let Some(index) = self.tool_index(data) else {
            return Vec::new();
        };

        vec![Event::ToolInput {
            index,
            delta: text_of(data, "delta"),
        }]
    }

    fn tool_index(&self, data: &Value) -> Option<usize> {
        let item = text_of(data, "item_id");
        if !item.is_empty()
            && let Some(index) = self.tools.iter().position(|tool| tool.item == item)
        {
            return Some(index);
        }

        let at = output_index(data)?;
        self.tools.iter().position(|tool| tool.at == Some(at))
    }

    fn item_done(&mut self, data: &Value) -> Vec<Event> {
        let Some(item) = data.get("item") else {
            return Vec::new();
        };
        if self.output.len() < MAX_ITEMS {
            self.output.push(item.clone());
        }
        if text_of(item, "type") != "function_call" {
            return Vec::new();
        }

        decode::call(item)
            .map(Event::ToolCall)
            .into_iter()
            .collect()
    }

    fn completed(&mut self, data: &Value) -> Result<Vec<Event>> {
        let Some(response) = data.get("response") else {
            return Err(Error::Malformed(
                "the stream finished without the answer".to_owned(),
            ));
        };
        let gathered = self.result_item(response);
        let answer = gathered.as_ref().unwrap_or(response);

        Ok(vec![Event::Done(Box::new(decode::response(
            answer,
            self.vendor,
        )?))])
    }

    fn result_item(&mut self, response: &Value) -> Option<Value> {
        let has_field = |field| !text_of(response, field).is_empty();
        let listed = response
            .get("output")
            .and_then(Value::as_array)
            .is_some_and(|output| !output.is_empty());

        if has_field("id") && has_field("model") && listed {
            return None;
        }

        let mut answer = response.clone();
        if !has_field("id") {
            answer["id"] = json!(self.id);
        }
        if !has_field("model") {
            answer["model"] = json!(self.model);
        }
        if !listed {
            answer["output"] = Value::Array(std::mem::take(&mut self.output));
        }
        Some(answer)
    }
}

fn output_index(data: &Value) -> Option<u64> {
    data.get("output_index").and_then(Value::as_u64)
}

fn failed(data: &Value) -> Error {
    let message = data
        .get("response")
        .and_then(|response| response.get("error"))
        .map(|error| text_of(error, "message"))
        .unwrap_or_default();

    Error::Rejected(message)
}
