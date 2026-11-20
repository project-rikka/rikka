use std::cmp::Ordering;

use rikka_model_core::NodeId;

#[derive(Clone, Debug)]
pub(crate) struct Lineage {
    above: Vec<Option<NodeId>>,
    depth: Vec<u32>,
}

impl Lineage {
    pub(crate) fn new(above: Vec<Option<NodeId>>, depth: Vec<u32>) -> Self {
        Self { above, depth }
    }

    pub(crate) fn above(&self, node: NodeId) -> Option<NodeId> {
        self.above[node.index()]
    }

    pub(crate) fn depth(&self, node: NodeId) -> u32 {
        self.depth[node.index()]
    }

    pub(crate) fn contains(&self, ancestor: NodeId, node: NodeId) -> bool {
        let mut cursor = Some(node);
        while let Some(current) = cursor {
            if current == ancestor {
                return true;
            }
            if self.depth(current) <= self.depth(ancestor) {
                return false;
            }
            cursor = self.above(current);
        }
        false
    }

    pub(crate) fn below(&self, ancestor: NodeId, node: NodeId) -> bool {
        ancestor != node && self.contains(ancestor, node)
    }

    pub(crate) fn common(&self, a: NodeId, b: NodeId) -> Option<NodeId> {
        let (mut a, mut b) = (a, b);
        loop {
            if a == b {
                return Some(a);
            }
            match self.depth(a).cmp(&self.depth(b)) {
                Ordering::Greater => a = self.above(a)?,
                Ordering::Less => b = self.above(b)?,
                Ordering::Equal => {
                    a = self.above(a)?;
                    b = self.above(b)?;
                }
            }
        }
    }

    pub(crate) fn path(&self, from: NodeId, to: NodeId) -> Vec<NodeId> {
        let mut path = Vec::new();
        let mut cursor = Some(to);
        while let Some(current) = cursor {
            if current == from {
                path.reverse();
                return path;
            }
            path.push(current);
            cursor = self.above(current);
        }
        Vec::new()
    }
}
