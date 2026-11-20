pub mod anthropic;
pub mod cloud_code;
pub mod codex;
pub mod openai;

use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use serde_json::Value;

use crate::effort::Effort;
use crate::server_tool::ServerTool;
use crate::thinking_off::ThinkingOff;

#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedModel {
    id: String,
    name: Option<String>,
    context: Option<u64>,
    output: Option<u32>,
    vision: Option<bool>,
    efforts: Option<Vec<Effort>>,
    thinking_off: Option<ThinkingOff>,
    server_tools: Option<Vec<ServerTool>>,
    released: Option<Date>,
    deprecated: bool,
}

impl ListedModel {
    pub fn new(id: impl Into<String>) -> ListedModel {
        ListedModel {
            id: id.into(),
            name: None,
            context: None,
            output: None,
            vision: None,
            efforts: None,
            thinking_off: None,
            server_tools: None,
            released: None,
            deprecated: false,
        }
    }

    pub fn with_name(mut self, name: Option<String>) -> Self {
        self.name = name;
        self
    }

    pub fn with_context(mut self, context: Option<u64>) -> Self {
        self.context = context;
        self
    }

    pub fn with_output(mut self, output: Option<u32>) -> Self {
        self.output = output;
        self
    }

    pub fn with_vision(mut self, vision: Option<bool>) -> Self {
        self.vision = vision;
        self
    }

    pub fn with_efforts(mut self, efforts: Option<Vec<Effort>>) -> Self {
        self.efforts = efforts.map(ordered);
        self
    }

    pub fn with_thinking_off(mut self, thinking_off: Option<ThinkingOff>) -> Self {
        self.thinking_off = thinking_off;
        self
    }

    pub fn with_server_tools(mut self, server_tools: Option<Vec<ServerTool>>) -> Self {
        self.server_tools = server_tools;
        self
    }

    pub fn with_released(mut self, released: Option<Date>) -> Self {
        self.released = released;
        self
    }

    pub fn with_deprecated(mut self, deprecated: bool) -> Self {
        self.deprecated = deprecated;
        self
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    #[must_use]
    pub fn context(&self) -> Option<u64> {
        self.context
    }

    #[must_use]
    pub fn output(&self) -> Option<u32> {
        self.output
    }

    #[must_use]
    pub fn vision(&self) -> Option<bool> {
        self.vision
    }

    #[must_use]
    pub fn efforts(&self) -> Option<&[Effort]> {
        self.efforts.as_deref()
    }

    #[must_use]
    pub fn thinking_off(&self) -> Option<ThinkingOff> {
        self.thinking_off
    }

    #[must_use]
    pub fn server_tools(&self) -> Option<&[ServerTool]> {
        self.server_tools.as_deref()
    }

    #[must_use]
    pub fn released(&self) -> Option<Date> {
        self.released
    }

    #[must_use]
    pub fn is_deprecated(&self) -> bool {
        self.deprecated
    }
}

fn ordered(mut efforts: Vec<Effort>) -> Vec<Effort> {
    efforts.sort_unstable();
    efforts.dedup();
    efforts
}

fn text(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn tokens(value: &Value, field: &str) -> Option<u64> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .filter(|tokens| *tokens > 0)
}

fn output_tokens(value: &Value, field: &str) -> Option<u32> {
    tokens(value, field).and_then(|tokens| u32::try_from(tokens).ok())
}

fn efforts<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<Effort> {
    names.into_iter().filter_map(Effort::from_id).collect()
}

fn day_of_unix(seconds: i64) -> Option<Date> {
    Timestamp::from_second(seconds)
        .ok()
        .map(|moment| moment.to_zoned(TimeZone::UTC).date())
}

fn day_of_rfc3339(text: &str) -> Option<Date> {
    text.parse::<Timestamp>()
        .ok()
        .map(|moment| moment.to_zoned(TimeZone::UTC).date())
}
