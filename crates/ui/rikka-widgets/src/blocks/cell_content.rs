use std::cell::Cell;
use std::rc::Rc;

use rikka_ui::{Element, Reads};

use crate::lists::collection::Slot;

pub(crate) type Shared = Rc<dyn Fn(Slot) -> Element>;

pub(crate) struct PrebuiltCell {
    element: Element,
    reads: Reads,
}

impl PrebuiltCell {
    pub(crate) fn build(cell: &Shared, slot: Slot) -> PrebuiltCell {
        let (element, reads) = Reads::record(|| cell(slot));
        PrebuiltCell { element, reads }
    }

    pub(crate) fn key(&self) -> Option<u64> {
        self.element.key()
    }
}

pub(crate) struct Lazily {
    digest: u64,
    primed: Cell<Option<PrebuiltCell>>,
    cell: Shared,
    slot: Slot,
}

impl Lazily {
    pub(crate) fn digest(&self) -> u64 {
        self.digest
    }

    pub(crate) fn build(&self) -> Element {
        if let Some(primed) = self.primed.take() {
            primed.reads.replay();
            return primed.element;
        }
        (self.cell)(self.slot)
    }
}

pub(crate) enum CellContent {
    Built(Element),
    Lazily(Lazily),
}

impl CellContent {
    pub(crate) fn cell(
        digest: u64,
        cell: Shared,
        slot: Slot,
        primed: Option<PrebuiltCell>,
    ) -> CellContent {
        CellContent::Lazily(Lazily {
            digest,
            primed: Cell::new(primed),
            cell,
            slot,
        })
    }

    pub(crate) fn parts(self) -> (Vec<Element>, Option<Lazily>) {
        match self {
            CellContent::Built(child) => (vec![child], None),
            CellContent::Lazily(lazily) => (Vec::new(), Some(lazily)),
        }
    }
}
