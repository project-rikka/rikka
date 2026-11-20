use std::collections::BTreeSet;

use rikka_types::Modifiers;

use crate::controls::node_canvas::model::{CanvasLink, FrameKey, NodeKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Item {
    Node(NodeKey),
    Frame(FrameKey),
    Link(CanvasLink),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extend {
    Replace,
    Add,
    Toggle,
}

impl Extend {
    pub fn from_modifiers(modifiers: Modifiers) -> Self {
        match (modifiers.control, modifiers.shift) {
            (true, _) => Self::Toggle,
            (false, true) => Self::Add,
            (false, false) => Self::Replace,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub items: BTreeSet<Item>,
}

impl Selection {
    pub fn of(items: impl IntoIterator<Item = Item>) -> Self {
        Self {
            items: items.into_iter().collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn contains(&self, item: Item) -> bool {
        self.items.contains(&item)
    }

    pub fn nodes(&self) -> impl Iterator<Item = NodeKey> + '_ {
        self.items.iter().filter_map(|item| match item {
            Item::Node(key) => Some(*key),
            _ => None,
        })
    }

    pub fn frames(&self) -> impl Iterator<Item = FrameKey> + '_ {
        self.items.iter().filter_map(|item| match item {
            Item::Frame(key) => Some(*key),
            _ => None,
        })
    }

    pub fn links(&self) -> impl Iterator<Item = CanvasLink> + '_ {
        self.items.iter().filter_map(|item| match item {
            Item::Link(link) => Some(*link),
            _ => None,
        })
    }

    pub fn picked(&self, item: Item, extend: Extend) -> Self {
        let mut items = match extend {
            Extend::Replace => BTreeSet::new(),
            Extend::Add | Extend::Toggle => self.items.clone(),
        };
        if extend == Extend::Toggle && items.contains(&item) {
            items.remove(&item);
        } else {
            items.insert(item);
        }
        Self { items }
    }

    pub fn pressed(&self, item: Item, extend: Extend) -> Self {
        match extend == Extend::Replace && self.contains(item) {
            true => self.clone(),
            false => self.picked(item, extend),
        }
    }

    pub fn boxed(&self, inside: impl IntoIterator<Item = Item>, extend: Extend) -> Self {
        let inside: BTreeSet<Item> = inside.into_iter().collect();
        let items = match extend {
            Extend::Replace => inside,
            Extend::Add => self.items.union(&inside).copied().collect(),
            Extend::Toggle => self.items.symmetric_difference(&inside).copied().collect(),
        };
        Self { items }
    }

    pub fn retained(&self, exists: impl Fn(Item) -> bool) -> Self {
        Self {
            items: self
                .items
                .iter()
                .copied()
                .filter(|item| exists(*item))
                .collect(),
        }
    }
}
