use super::edges::{X, Y, scrolls};
use super::{BELOW, DIRTY, Layouts, NO_PARENT, Node, QUEUED};

#[derive(Clone, Copy, PartialEq)]
struct Facade {
    leaf: bool,
    lane: f32,
    size: [f32; 2],
    visible: [f32; 2],
}

impl Layouts {
    pub(super) fn is_boundary(&self, node: Node) -> bool {
        let at = node.index();
        let parent = self.parent[at];
        if parent == NO_PARENT {
            return false;
        }
        let style = &self.style[at];
        style.inset.is_none()
            && style.size.width.fixed().is_some()
            && style.size.height.fixed().is_some()
            && self.style[parent as usize].scrolls.is_none()
    }

    pub(super) fn queue_boundary(&mut self, node: Node) {
        let flags = &mut self.flags[node.index()];
        if *flags & QUEUED == 0 {
            *flags |= QUEUED;
            self.boundaries.push(node);
        }
    }

    pub(crate) fn boundaries(&self) -> &[Node] {
        &self.boundaries
    }

    pub(super) fn settle_boundaries(&mut self) {
        while let Some(node) = self.boundaries.pop() {
            let at = node.index();
            if self.flags[at] & QUEUED == 0 {
                continue;
            }
            self.flags[at] &= !QUEUED;
            if self.flags[at] & (DIRTY | BELOW) != 0 {
                self.settle_boundary(node);
            }
        }
    }

    fn settle_boundary(&mut self, node: Node) {
        let at = node.index();
        let parent = self.parent[at];
        let laid = self.laid[at];
        let fresh = laid.width.is_nan() || laid.height.is_nan() || !self.was_laid(node);
        if fresh || !self.is_boundary(node) {
            self.climb(parent);
            return;
        }
        let before = self.facade(node);
        self.intrinsic(node);
        self.lay_width(node, laid.width);
        self.measure_heights(node);
        self.place(node, self.size[at][X], laid.height);
        match self.facade(node) == before {
            true => self.round(node, self.world[parent as usize]),
            false => self.climb(parent),
        }
    }

    fn facade(&self, node: Node) -> Facade {
        let at = node.index();
        let style = &self.style[at];
        let size = self.size[at];
        let content = self.content[at];
        let visible = |axis: usize| match scrolls(style, axis) {
            true => size[axis],
            false => size[axis].max(content[axis]),
        };
        Facade {
            leaf: self.children[at].is_empty(),
            lane: self.lane[at],
            size,
            visible: [visible(X), visible(Y)],
        }
    }
}
