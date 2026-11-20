use rikka_ui::Space;

use crate::navigation::toolbar::item::ToolItem;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolPriority {
    Low,
    #[default]
    Normal,
    High,
    Pinned,
}

#[must_use]
#[derive(Debug)]
pub struct ToolGroup {
    pub(super) items: Vec<ToolItem>,
    pub(super) priority: ToolPriority,
    pub(super) gap: Option<f32>,
}

pub fn tool_group(items: impl IntoIterator<Item = impl Into<ToolItem>>) -> ToolGroup {
    ToolGroup {
        items: items.into_iter().map(Into::into).collect(),
        priority: ToolPriority::default(),
        gap: None,
    }
}

impl ToolGroup {
    pub fn priority(mut self, priority: ToolPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn gap(mut self, gap: impl Into<Space>) -> Self {
        self.gap = Some(gap.into().px());
        self
    }
}
