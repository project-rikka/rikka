use crate::document::Document;
use crate::hosted::Hosted;
use crate::reasoning::Reasoning;
use crate::text::Text;
use crate::tool_result::ToolResult;
use crate::tool_use::ToolUse;
use rikka_tool::Image;

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Content {
    Text(Text),
    Image(Image),
    Document(Document),
    ToolUse(ToolUse),
    ToolResult(ToolResult),
    Reasoning(Reasoning),
    Hosted(Hosted),
}

impl Content {
    #[must_use]
    pub fn text(text: impl Into<String>) -> Content {
        Content::Text(Text::new(text))
    }

    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Content::Text(text) => Some(text.as_str()),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_tool_use(&self) -> Option<&ToolUse> {
        match self {
            Content::ToolUse(call) => Some(call),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_reasoning(&self) -> Option<&Reasoning> {
        match self {
            Content::Reasoning(reasoning) => Some(reasoning),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_hosted(&self) -> Option<&Hosted> {
        match self {
            Content::Hosted(hosted) => Some(hosted),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_cached(&self) -> bool {
        match self {
            Content::Text(text) => text.is_cached(),
            Content::Image(image) => image.is_cached(),
            Content::Document(document) => document.is_cached(),
            Content::ToolResult(result) => result.is_cached(),
            Content::ToolUse(_) | Content::Reasoning(_) | Content::Hosted(_) => false,
        }
    }
}

pub trait IntoContent {
    #[must_use]
    fn into_content(self) -> Vec<Content>;
}

impl IntoContent for Content {
    fn into_content(self) -> Vec<Content> {
        vec![self]
    }
}

impl IntoContent for Vec<Content> {
    fn into_content(self) -> Vec<Content> {
        self
    }
}

impl<const N: usize> IntoContent for [Content; N] {
    fn into_content(self) -> Vec<Content> {
        self.into()
    }
}

impl IntoContent for &str {
    fn into_content(self) -> Vec<Content> {
        vec![Content::text(self)]
    }
}

impl IntoContent for String {
    fn into_content(self) -> Vec<Content> {
        vec![Content::text(self)]
    }
}

macro_rules! block {
    ($type:ty, $variant:ident) => {
        impl From<$type> for Content {
            fn from(value: $type) -> Content {
                Content::$variant(value)
            }
        }

        impl IntoContent for $type {
            fn into_content(self) -> Vec<Content> {
                vec![Content::$variant(self)]
            }
        }
    };
}

block!(Text, Text);
block!(Image, Image);
block!(Document, Document);
block!(ToolUse, ToolUse);
block!(ToolResult, ToolResult);
block!(Reasoning, Reasoning);
block!(Hosted, Hosted);
