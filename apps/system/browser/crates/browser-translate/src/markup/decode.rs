use rikka_html::{Token, tokens};

use crate::markup::dialect::Dialect;
use crate::markup::plan::Plan;

const VOID_TAGS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track",
    "wbr",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Parent {
    Root,
    Slot(usize),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Item {
    Text(String),
    Slot(usize),
}

#[derive(Debug)]
pub(crate) struct Arena {
    pub(crate) root: Vec<Item>,
    pub(crate) slots: Vec<Option<Vec<Item>>>,
    pub(crate) parents: Vec<Option<Parent>>,
}

impl Arena {
    fn new(slots: usize) -> Arena {
        Arena {
            root: Vec::new(),
            slots: (0..slots).map(|_| None).collect(),
            parents: vec![None; slots],
        }
    }

    pub(crate) fn items(&self, parent: Parent) -> &[Item] {
        match parent {
            Parent::Root => &self.root,
            Parent::Slot(slot) => self.slots[slot].as_deref().unwrap_or_default(),
        }
    }

    pub(crate) fn items_mut(&mut self, parent: Parent) -> &mut Vec<Item> {
        match parent {
            Parent::Root => &mut self.root,
            Parent::Slot(slot) => self.slots[slot].get_or_insert_with(Vec::new),
        }
    }

    fn push_text(&mut self, parent: Parent, text: &str) {
        let items = self.items_mut(parent);
        if let Some(Item::Text(last)) = items.last_mut() {
            last.push_str(text);
        } else {
            items.push(Item::Text(text.to_owned()));
        }
    }
}

pub(crate) fn decode(html: &str, plan: &Plan, dialect: &Dialect) -> Arena {
    let mut arena = Arena::new(plan.slots.len());
    let mut frames: Vec<(String, Option<Parent>)> = Vec::new();
    for token in tokens(html) {
        let current = frames
            .iter()
            .rev()
            .find_map(|(_, parent)| *parent)
            .unwrap_or(Parent::Root);
        match token {
            Token::Text(text) => arena.push_text(current, &text),
            Token::Close(name) => {
                if let Some(at) = frames.iter().rposition(|(open, _)| *open == name) {
                    frames.truncate(at);
                }
            }
            Token::Open(tag) => {
                if VOID_TAGS.contains(&tag.name()) {
                    continue;
                }
                let slot = tag
                    .attribute(dialect.id)
                    .and_then(|value| value.trim().parse::<u32>().ok())
                    .and_then(|id| plan.slot_of(id));
                let parent = slot.map(|slot| {
                    if arena.slots[slot].is_none() {
                        arena.slots[slot] = Some(Vec::new());
                        arena.parents[slot] = Some(current);
                        arena.items_mut(current).push(Item::Slot(slot));
                    }
                    Parent::Slot(slot)
                });
                if !tag.is_self_closing() {
                    frames.push((tag.name().to_owned(), parent));
                }
            }
        }
    }
    arena
}
