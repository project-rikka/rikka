use crate::effort::Effort;
use crate::price::Price;
use crate::server_tool::ServerTool;
use crate::thinking_off::ThinkingOff;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ModelInfo {
    id: String,
    name: String,
    context: u64,
    output: u32,
    vision: bool,
    #[serde(default = "capable")]
    tools: bool,
    efforts: Vec<Effort>,
    #[serde(default, skip_serializing_if = "ThinkingOff::is_disabled")]
    thinking_off: ThinkingOff,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    server_tools: Vec<ServerTool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    price: Option<Price>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    superseded: bool,
}

impl ModelInfo {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        context: u64,
        output: u32,
    ) -> ModelInfo {
        ModelInfo {
            id: id.into(),
            name: name.into(),
            context,
            output,
            vision: false,
            tools: true,
            efforts: Vec::new(),
            thinking_off: ThinkingOff::Disabled,
            server_tools: Vec::new(),
            price: None,
            superseded: false,
        }
    }

    #[must_use]
    pub fn with_vision(mut self, vision: bool) -> Self {
        self.vision = vision;
        self
    }

    #[must_use]
    pub fn with_tools(mut self, tools: bool) -> Self {
        self.tools = tools;
        self
    }

    #[must_use]
    pub fn with_efforts(mut self, efforts: Vec<Effort>) -> Self {
        self.efforts = efforts;
        self
    }

    #[must_use]
    pub fn with_thinking_off(mut self, thinking_off: ThinkingOff) -> Self {
        self.thinking_off = thinking_off;
        self
    }

    #[must_use]
    pub fn with_server_tools(mut self, server_tools: Vec<ServerTool>) -> Self {
        self.server_tools = server_tools;
        self
    }

    #[must_use]
    pub fn with_price(mut self, price: Option<Price>) -> Self {
        self.price = price;
        self
    }

    #[must_use]
    pub fn with_superseded(mut self, superseded: bool) -> Self {
        self.superseded = superseded;
        self
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn context(&self) -> u64 {
        self.context
    }

    #[must_use]
    pub fn output(&self) -> u32 {
        self.output
    }

    #[must_use]
    pub fn vision(&self) -> bool {
        self.vision
    }

    #[must_use]
    pub fn tools(&self) -> bool {
        self.tools
    }

    #[must_use]
    pub fn efforts(&self) -> &[Effort] {
        &self.efforts
    }

    #[must_use]
    pub fn thinking_off(&self) -> ThinkingOff {
        self.thinking_off
    }

    #[must_use]
    pub fn server_tools(&self) -> &[ServerTool] {
        &self.server_tools
    }

    #[must_use]
    pub fn price(&self) -> Option<Price> {
        self.price
    }

    #[must_use]
    pub fn is_superseded(&self) -> bool {
        self.superseded
    }

    #[must_use]
    pub fn nearest_effort(&self, requested: Effort) -> Option<Effort> {
        self.efforts
            .iter()
            .rev()
            .find(|effort| **effort <= requested)
            .or_else(|| self.efforts.first())
            .copied()
    }
}

fn capable() -> bool {
    true
}
