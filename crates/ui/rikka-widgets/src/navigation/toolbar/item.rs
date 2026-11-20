use rikka_ui::{Element, View};

use crate::navigation::menu::MenuItem;

#[must_use]
pub struct ToolItem {
    pub(super) content: Element,
    pub(super) overflow: Vec<MenuItem>,
}

pub fn tool_item(content: impl View) -> ToolItem {
    ToolItem {
        content: content.build(),
        overflow: Vec::new(),
    }
}

impl ToolItem {
    pub fn overflow(mut self, items: impl IntoIterator<Item = MenuItem>) -> Self {
        self.overflow = items.into_iter().collect();
        self
    }
}

impl std::fmt::Debug for ToolItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolItem")
            .field("overflow", &self.overflow.len())
            .finish_non_exhaustive()
    }
}
