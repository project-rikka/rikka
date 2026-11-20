#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodePath {
    parent: Vec<usize>,
    index: usize,
}

impl NodePath {
    pub fn root(index: usize) -> NodePath {
        NodePath {
            parent: Vec::new(),
            index,
        }
    }

    pub fn parent(&self) -> &[usize] {
        &self.parent
    }

    pub fn index(&self) -> usize {
        self.index
    }

    #[must_use]
    pub fn sibling(&self, index: usize) -> NodePath {
        NodePath {
            parent: self.parent.clone(),
            index,
        }
    }

    #[must_use]
    pub fn child(&self, index: usize) -> NodePath {
        let mut parent = self.parent.clone();
        parent.push(self.index);
        NodePath { parent, index }
    }

    pub fn group(&self) -> Option<NodePath> {
        let (&index, parent) = self.parent.split_last()?;
        Some(NodePath {
            parent: parent.to_vec(),
            index,
        })
    }

    pub fn holds(&self, other: &NodePath) -> bool {
        other.parent.len() > self.parent.len()
            && other.parent[..self.parent.len()] == self.parent[..]
            && other.parent[self.parent.len()] == self.index
    }

    #[must_use]
    pub fn after_removal(mut self, removed: &NodePath) -> NodePath {
        let depth = removed.parent.len();
        if self.parent.len() < depth || self.parent[..depth] != removed.parent[..] {
            return self;
        }
        let at = match self.parent.get_mut(depth) {
            Some(at) => at,
            None => &mut self.index,
        };
        if *at > removed.index {
            *at -= 1;
        }
        self
    }

    pub(crate) fn from_parts(parent: Vec<usize>, index: usize) -> NodePath {
        NodePath { parent, index }
    }
}
