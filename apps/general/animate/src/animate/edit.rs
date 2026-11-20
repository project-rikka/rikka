use std::sync::Arc;

use animate_edit::{Change, Undoable};
use rikka_physics_tailor::ClothSettings;

use crate::animate::subject_motion::SubjectMotion;

#[derive(Clone, Debug)]
pub(crate) struct Pair<T> {
    before: T,
    after: T,
}

impl<T> Pair<T> {
    pub(crate) fn new(before: T, after: T) -> Self {
        Self { before, after }
    }

    pub(crate) fn toward(&self, forward: bool) -> &T {
        match forward {
            true => &self.after,
            false => &self.before,
        }
    }
}

impl<T: PartialEq> Pair<T> {
    pub(crate) fn is_empty(&self) -> bool {
        self.before == self.after
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Edit {
    Keys(Change),
    Replaced(Pair<Option<SubjectMotion>>),
    Cloth(Pair<Arc<ClothSettings>>),
}

impl Edit {
    pub(crate) fn merge(held: &mut Self, next: Self) {
        if let (Self::Replaced(held), Self::Replaced(next)) = (held, next) {
            held.after = next.after;
        }
    }
}

impl Undoable for Edit {
    fn is_empty(&self) -> bool {
        match self {
            Self::Keys(change) => change.is_empty(),
            Self::Replaced(pair) => pair.is_empty(),
            Self::Cloth(pair) => pair.is_empty(),
        }
    }
}
