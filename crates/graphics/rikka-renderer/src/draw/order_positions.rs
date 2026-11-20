use rustc_hash::FxHashMap;

use crate::scene::NodeId;

#[derive(Debug, Default)]
pub(crate) struct OrderPositions {
    by_node: FxHashMap<NodeId, usize>,
}

impl OrderPositions {
    pub(crate) fn index(&mut self, order: &[NodeId]) {
        self.by_node.clear();
        self.by_node
            .extend(order.iter().enumerate().map(|(at, id)| (*id, at)));
    }

    pub(crate) fn clear(&mut self) {
        self.by_node.clear();
    }

    pub(crate) fn find(&self, order: &[NodeId], id: NodeId) -> Option<usize> {
        match self.by_node.is_empty() {
            true => order.iter().position(|other| *other == id),
            false => self.by_node.get(&id).copied(),
        }
    }
}
