use rikka_types::{Rect, Size};

use super::{Layouts, Node};
use crate::paint::MeasureSpec;
use crate::style::Layout;

#[derive(Default)]
pub struct LayoutTree {
    layouts: Layouts,
}

impl LayoutTree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn container(&mut self, layout: Layout) -> Node {
        self.layouts.create(layout, None)
    }

    pub fn leaf(&mut self, layout: Layout, measure: MeasureSpec) -> Node {
        self.layouts.create(layout, Some(measure))
    }

    pub fn set_children(&mut self, node: Node, children: &[Node]) {
        self.layouts.set_children(node, children);
    }

    pub fn set_layout(&mut self, node: Node, layout: Layout) {
        self.layouts.set_style(node, layout);
    }

    pub fn set_measure(&mut self, node: Node, measure: Option<MeasureSpec>) {
        self.layouts.set_measure(node, measure);
    }

    pub fn children(&self, node: Node) -> &[Node] {
        self.layouts.children(node)
    }

    pub fn solve(&mut self, root: Node, viewport: Size) {
        self.layouts.solve(root, viewport, false);
        self.layouts.moved.clear();
    }

    pub fn rect(&self, node: Node) -> Rect {
        self.layouts.rect(node)
    }
}

impl std::fmt::Debug for LayoutTree {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("LayoutTree").finish_non_exhaustive()
    }
}
