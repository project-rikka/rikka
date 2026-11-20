use serde::Serialize;
use serde_json::Value;

use rikka_tool::Image;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResult {
    id: String,
    output: Vec<Output>,
    failed: bool,
    cached: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    Text(String),
    Image(Image),
    Reference(String),
}

impl ToolResult {
    #[must_use]
    pub fn new(id: impl Into<String>, output: impl Into<String>) -> ToolResult {
        ToolResult {
            id: id.into(),
            output: vec![Output::Text(output.into())],
            failed: false,
            cached: false,
        }
    }

    #[must_use]
    pub fn json(id: impl Into<String>, value: &impl Serialize) -> ToolResult {
        match serde_json::to_string(value) {
            Ok(text) => ToolResult::new(id, text),
            Err(error) => ToolResult::failed(id, error.to_string()),
        }
    }

    #[must_use]
    pub fn failed(id: impl Into<String>, reason: impl Into<String>) -> ToolResult {
        ToolResult {
            failed: true,
            ..ToolResult::new(id, reason)
        }
    }

    #[must_use]
    pub fn image(mut self, image: Image) -> Self {
        self.output.push(Output::Image(image));
        self
    }

    #[must_use]
    pub fn reference(mut self, tool: impl Into<String>) -> Self {
        self.output.push(Output::Reference(tool.into()));
        self
    }

    #[must_use]
    pub fn cached(mut self) -> Self {
        self.cached = true;
        self
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn output(&self) -> &[Output] {
        &self.output
    }

    #[must_use]
    pub fn is_failed(&self) -> bool {
        self.failed
    }

    #[must_use]
    pub fn is_cached(&self) -> bool {
        self.cached
    }

    #[must_use]
    pub fn text(&self) -> String {
        self.output
            .iter()
            .filter_map(|output| match output {
                Output::Text(text) => Some(text.as_str()),
                Output::Image(_) | Output::Reference(_) => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[must_use]
    pub fn has_images(&self) -> bool {
        self.output
            .iter()
            .any(|output| matches!(output, Output::Image(_)))
    }

    #[must_use]
    pub fn replacing_images(&self, placeholder: &str) -> ToolResult {
        ToolResult {
            output: self
                .output
                .iter()
                .map(|output| match output {
                    Output::Image(_) => Output::Text(placeholder.to_owned()),
                    kept => kept.clone(),
                })
                .collect(),
            id: self.id.clone(),
            ..*self
        }
    }

    pub fn references(&self) -> impl Iterator<Item = &str> {
        self.output.iter().filter_map(|output| match output {
            Output::Reference(tool) => Some(tool.as_str()),
            Output::Text(_) | Output::Image(_) => None,
        })
    }
}
impl From<Value> for Output {
    fn from(value: Value) -> Output {
        Output::Text(value.to_string())
    }
}
