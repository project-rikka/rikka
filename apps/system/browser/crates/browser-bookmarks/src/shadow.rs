use std::collections::HashMap;
use std::hash::Hash;

#[derive(Clone, Debug)]
pub(crate) struct Shadow<P, C> {
    children: HashMap<P, Vec<C>>,
}

impl<P: Clone + Eq + Hash, C: Clone + PartialEq> Shadow<P, C> {
    pub(crate) fn new(children: HashMap<P, Vec<C>>) -> Shadow<P, C> {
        Shadow { children }
    }

    pub(crate) fn len(&self, parent: &P) -> usize {
        self.children.get(parent).map_or(0, Vec::len)
    }

    pub(crate) fn locate(&self, child: &C) -> Option<(P, usize)> {
        self.children.iter().find_map(|(parent, children)| {
            children
                .iter()
                .position(|candidate| candidate == child)
                .map(|at| (parent.clone(), at))
        })
    }

    pub(crate) fn index(&self, parent: &P, child: &C) -> Option<usize> {
        self.children
            .get(parent)?
            .iter()
            .position(|candidate| candidate == child)
    }

    pub(crate) fn after(&self, parent: &P, previous: Option<&C>) -> usize {
        previous
            .and_then(|previous| self.index(parent, previous))
            .map_or(0, |at| at + 1)
    }

    pub(crate) fn insert(&mut self, parent: P, at: usize, child: C) {
        let children = self.children.entry(parent).or_default();
        children.insert(at.min(children.len()), child);
    }

    pub(crate) fn remove(&mut self, child: &C) -> Option<(P, usize)> {
        let (parent, at) = self.locate(child)?;
        if let Some(children) = self.children.get_mut(&parent) {
            children.remove(at);
        }
        Some((parent, at))
    }

    pub(crate) fn out_of_order(&self, parent: &P, child: &C, previous: Option<&C>) -> bool {
        match self.locate(child) {
            Some((current, at)) if current == *parent => previous
                .and_then(|previous| self.index(parent, previous))
                .is_some_and(|before| at < before),
            _ => true,
        }
    }
}
