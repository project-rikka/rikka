use std::collections::BTreeSet;

use cut_project::{Attribute, ClipId, Edit, KeyFit, Piece};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Pasting {
    pub(crate) from: Piece,
    pub(crate) onto: Vec<ClipId>,
    pub(crate) offered: Vec<Attribute>,
    pub(crate) picked: BTreeSet<Attribute>,
    pub(crate) keys: KeyFit,
}

impl Pasting {
    pub(crate) fn new(from: Piece, onto: Vec<ClipId>, offered: Vec<Attribute>) -> Pasting {
        let picked = offered
            .iter()
            .copied()
            .filter(|attribute| attribute.is_effect())
            .collect();
        Pasting {
            from,
            onto,
            offered,
            picked,
            keys: KeyFit::default(),
        }
    }

    pub(crate) fn effects(&self) -> impl Iterator<Item = Attribute> + '_ {
        self.offered
            .iter()
            .copied()
            .filter(|attribute| attribute.is_effect())
    }

    pub(crate) fn others(&self) -> impl Iterator<Item = Attribute> + '_ {
        self.offered
            .iter()
            .copied()
            .filter(|attribute| !attribute.is_effect())
    }

    pub(crate) fn pick(&mut self, attribute: Attribute, on: bool) {
        match on {
            true => self.picked.insert(attribute),
            false => self.picked.remove(&attribute),
        };
    }

    pub(crate) fn pick_effects(&mut self, on: bool) {
        let effects: Vec<Attribute> = self.effects().collect();
        for attribute in effects {
            self.pick(attribute, on);
        }
    }

    pub(crate) fn edit(&self) -> Option<Edit> {
        if self.picked.is_empty() {
            return None;
        }
        Some(Edit::PasteAttributes {
            from: Box::new(self.from.clone()),
            onto: self.onto.clone(),
            attributes: self.picked.iter().copied().collect(),
            keys: self.keys,
        })
    }
}
