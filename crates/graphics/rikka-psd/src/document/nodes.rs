use crate::document::Document;
use crate::error::{Error, Result};
use crate::layers::tree::{self, Preorder};
use crate::layers::{Node, NodePath};

impl Document {
    pub fn preorder(&self) -> impl Iterator<Item = &Node> {
        Preorder::new(&self.layers)
    }

    pub fn preorder_path(&self, position: usize) -> Option<NodePath> {
        tree::preorder_path(&self.layers, position)
    }

    pub fn preorder_index(&self, path: &NodePath) -> Option<usize> {
        tree::preorder_index(&self.layers, path)
    }

    pub fn siblings(&self, path: &NodePath) -> Option<&[Node]> {
        tree::siblings(&self.layers, path.parent())
    }

    pub fn node(&self, path: &NodePath) -> Option<&Node> {
        self.siblings(path)?.get(path.index())
    }

    pub fn node_mut(&mut self, path: &NodePath) -> Option<&mut Node> {
        tree::siblings_mut(&mut self.layers, path.parent())?.get_mut(path.index())
    }

    pub fn remove_node(&mut self, path: &NodePath) -> Option<Node> {
        let siblings = tree::siblings_mut(&mut self.layers, path.parent())?;
        (path.index() < siblings.len()).then(|| siblings.remove(path.index()))
    }

    pub fn insert_node(&mut self, path: &NodePath, node: Node) -> Result<()> {
        let siblings = tree::siblings_mut(&mut self.layers, path.parent())
            .filter(|siblings| path.index() <= siblings.len())
            .ok_or(Error::NoPlace)?;
        siblings.insert(path.index(), node);
        Ok(())
    }
}
