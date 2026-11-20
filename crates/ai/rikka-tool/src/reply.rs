use serde_json::Value;

use crate::image::Image;
use crate::usage::Usage;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reply {
    text: String,
    images: Vec<Image>,
    displayed: Vec<Image>,
    loaded_tools: Vec<String>,
    detail: Option<Value>,
    usage: Usage,
    failed: bool,
}

#[derive(Debug)]
pub struct ReplyParts {
    pub text: String,
    pub images: Vec<Image>,
    pub displayed: Vec<Image>,
    pub loaded_tools: Vec<String>,
    pub detail: Option<Value>,
    pub failed: bool,
}

impl Reply {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Reply {
        Reply {
            text: text.into(),
            images: Vec::new(),
            displayed: Vec::new(),
            loaded_tools: Vec::new(),
            detail: None,
            usage: Usage::default(),
            failed: false,
        }
    }

    #[must_use]
    pub fn json(value: &impl serde::Serialize) -> Reply {
        match serde_json::to_string(value) {
            Ok(text) => Reply::new(text),
            Err(error) => Reply::new(error.to_string()),
        }
    }

    #[must_use]
    pub fn image(mut self, image: Image) -> Self {
        self.images.push(image);
        self
    }

    #[must_use]
    pub fn display(mut self, image: Image) -> Self {
        self.displayed.push(image);
        self
    }

    #[must_use]
    pub fn load_tool(mut self, tool: impl Into<String>) -> Self {
        self.loaded_tools.push(tool.into());
        self
    }

    #[must_use]
    pub fn detail(mut self, detail: Value) -> Self {
        self.detail = Some(detail);
        self
    }

    #[must_use]
    pub fn failed(mut self) -> Self {
        self.failed = true;
        self
    }

    #[must_use]
    pub fn with_usage(mut self, usage: Usage) -> Self {
        self.usage = usage;
        self
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
    #[must_use]
    pub fn images(&self) -> &[Image] {
        &self.images
    }

    #[must_use]
    pub fn displayed_images(&self) -> &[Image] {
        &self.displayed
    }

    #[must_use]
    pub fn loaded_tools(&self) -> &[String] {
        &self.loaded_tools
    }

    #[must_use]
    pub fn is_failed(&self) -> bool {
        self.failed
    }

    #[must_use]
    pub fn usage(&self) -> Usage {
        self.usage
    }

    #[must_use]
    pub fn into_parts(self) -> ReplyParts {
        ReplyParts {
            text: self.text,
            images: self.images,
            displayed: self.displayed,
            loaded_tools: self.loaded_tools,
            detail: self.detail,
            failed: self.failed,
        }
    }
}

impl From<String> for Reply {
    fn from(text: String) -> Reply {
        Reply::new(text)
    }
}

impl From<&str> for Reply {
    fn from(text: &str) -> Reply {
        Reply::new(text)
    }
}

impl From<Value> for Reply {
    fn from(value: Value) -> Reply {
        match value {
            Value::String(text) => Reply::new(text),
            other => Reply::new(other.to_string()),
        }
    }
}

impl From<Image> for Reply {
    fn from(image: Image) -> Reply {
        Reply::default().image(image)
    }
}

impl From<()> for Reply {
    fn from((): ()) -> Reply {
        Reply::new("done")
    }
}
