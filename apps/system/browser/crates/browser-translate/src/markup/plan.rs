use std::collections::HashMap;

#[derive(Debug, Default)]
pub(crate) struct Plan {
    pub(crate) slots: Vec<Slot>,
    pub(crate) root: Vec<Piece>,
    index: HashMap<u32, usize>,
}

#[derive(Debug)]
pub(crate) struct Slot {
    pub(crate) id: u32,
    pub(crate) kind: Kind,
    pub(crate) parent: Option<usize>,
    pub(crate) previous: Option<usize>,
    pub(crate) children: Vec<Piece>,
    pub(crate) has_text: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Element,
    Keep,
    Token(String),
}

#[derive(Clone, Debug)]
pub(crate) enum Piece {
    Text(String),
    Slot(usize),
}

impl Plan {
    pub(crate) fn add(
        &mut self,
        id: u32,
        kind: Kind,
        parent: Option<usize>,
        previous: Option<usize>,
    ) -> usize {
        let index = self.slots.len();
        self.slots.push(Slot {
            id,
            kind,
            parent,
            previous,
            children: Vec::new(),
            has_text: false,
        });
        self.index.insert(id, index);
        index
    }

    pub(crate) fn slot_of(&self, id: u32) -> Option<usize> {
        self.index.get(&id).copied()
    }
}
