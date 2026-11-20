use crate::content::{Content, IntoContent};
use crate::role::Role;
use crate::text::Text;
use crate::tool_result::ToolResult;
use crate::tool_use::ToolUse;

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    role: Role,
    content: Vec<Content>,
}

impl Message {
    #[must_use]
    pub fn new(role: Role, content: impl IntoContent) -> Message {
        Message {
            role,
            content: content.into_content(),
        }
    }

    #[must_use]
    pub fn user(content: impl IntoContent) -> Message {
        Message::new(Role::User, content)
    }

    #[must_use]
    pub fn assistant(content: impl IntoContent) -> Message {
        Message::new(Role::Assistant, content)
    }

    #[must_use]
    pub fn system(content: impl IntoContent) -> Message {
        Message::new(Role::System, content)
    }

    #[must_use]
    pub fn tool_results(results: impl IntoIterator<Item = ToolResult>) -> Message {
        Message::new(
            Role::User,
            results
                .into_iter()
                .map(Content::ToolResult)
                .collect::<Vec<_>>(),
        )
    }

    #[must_use]
    pub fn with(mut self, block: impl Into<Content>) -> Self {
        self.content.push(block.into());
        self
    }

    #[must_use]
    pub fn role(&self) -> Role {
        self.role
    }

    #[must_use]
    pub fn content(&self) -> &[Content] {
        &self.content
    }

    #[must_use]
    pub fn into_content(self) -> Vec<Content> {
        self.content
    }

    #[must_use]
    pub fn text(&self) -> String {
        let mut text = String::new();
        for block in self.content.iter().filter_map(Content::as_text) {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(block);
        }
        text
    }

    pub fn tool_calls(&self) -> impl Iterator<Item = &ToolUse> {
        self.content.iter().filter_map(Content::as_tool_use)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    #[must_use]
    pub fn has_images(&self) -> bool {
        self.content.iter().any(|block| match block {
            Content::Image(_) => true,
            Content::ToolResult(result) => result.has_images(),
            _ => false,
        })
    }

    #[must_use]
    pub fn replacing_images(&self, placeholder: &str) -> Message {
        let content = self
            .content
            .iter()
            .map(|block| match block {
                Content::Image(image) => {
                    let text = Text::new(placeholder);
                    Content::Text(match image.is_cached() {
                        true => text.cached(),
                        false => text,
                    })
                }
                Content::ToolResult(result) => {
                    Content::ToolResult(result.replacing_images(placeholder))
                }
                other => other.clone(),
            })
            .collect();
        Message {
            role: self.role,
            content,
        }
    }
}
