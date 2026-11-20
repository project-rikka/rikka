use std::ops::Range;
use std::rc::Rc;

use rikka_ui::hash_of;
use rustc_hash::FxHashMap;

use crate::blocks::cell_content::{CellContent, PrebuiltCell};
use crate::lists::collection::{Cell, Memo, Slot};

#[derive(Clone)]
pub(crate) struct Cells {
    pub(crate) cell: Cell,
    pub(crate) memo: Option<Memo>,
    pub(crate) subject: u64,
    pub(crate) chosen: Option<usize>,
    pub(crate) made: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Built {
    key: u64,
    digest: u64,
    place: u64,
}

#[derive(Debug, Default)]
pub(crate) struct Keys {
    made: u64,
    built: FxHashMap<usize, Built>,
}

impl Keys {
    pub(crate) fn adopt(&mut self, made: u64, window: Range<usize>) {
        if self.made != made {
            self.made = made;
            self.built.clear();
            return;
        }
        self.built.retain(|index, _| window.contains(index));
    }

    fn known(&self, made: u64, index: usize) -> Option<Built> {
        self.built
            .get(&index)
            .copied()
            .filter(|_| self.made == made)
    }

    fn note(&mut self, made: u64, index: usize, built: Built) {
        if self.made == made {
            self.built.insert(index, built);
        }
    }
}

impl Cells {
    pub(crate) fn retained_key(&self, index: usize, place: u64, keys: &Keys) -> Option<u64> {
        let (_, _, digest) = self.versioned(index);
        keys.known(self.made, index)
            .filter(|known| known.digest == digest && known.place == place)
            .map(|known| known.key)
    }

    pub(crate) fn seat(&self, index: usize, place: u64, keys: &mut Keys) -> (CellContent, u64) {
        let (slot, version, digest) = self.versioned(index);
        let known = keys.known(self.made, index);
        let (content, key) = self.hold(slot, version.map(|version| version.key), digest, known);
        keys.note(self.made, index, Built { key, digest, place });
        (content, key)
    }

    pub(crate) fn content(&self, index: usize) -> CellContent {
        let (slot, version, digest) = self.versioned(index);
        self.hold(slot, version.map(|version| version.key), digest, None)
            .0
    }

    fn versioned(&self, index: usize) -> (Slot, Option<rikka_ui::Version>, u64) {
        let slot = Slot {
            index,
            selected: self.chosen == Some(index),
        };
        let version = self.memo.as_ref().and_then(|memo| memo(index));
        let digest = match version {
            Some(version) => hash_of((version.revision, slot.selected)),
            None => hash_of((self.made, slot.selected)),
        };
        (slot, version, digest)
    }

    fn hold(
        &self,
        slot: Slot,
        versioned: Option<u64>,
        digest: u64,
        known: Option<Built>,
    ) -> (CellContent, u64) {
        let cell = Rc::clone(&self.cell);
        match (versioned, known) {
            (Some(key), _) => (CellContent::cell(digest, cell, slot, None), key),
            (None, Some(known)) => (CellContent::cell(digest, cell, slot, None), known.key),
            (None, None) => {
                let primed = PrebuiltCell::build(&cell, slot);
                let key = primed
                    .key()
                    .unwrap_or_else(|| hash_of((self.subject, slot.index)));
                (CellContent::cell(digest, cell, slot, Some(primed)), key)
            }
        }
    }

    pub(crate) fn chooses(&self, index: usize) -> bool {
        self.chosen == Some(index)
    }
}

impl std::fmt::Debug for Cells {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cells")
            .field("subject", &self.subject)
            .field("chosen", &self.chosen)
            .finish_non_exhaustive()
    }
}
