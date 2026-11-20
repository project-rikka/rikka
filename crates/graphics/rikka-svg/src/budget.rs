use std::cell::Cell;

const FLOOR: usize = 16 << 20;
const PER_SOURCE_BYTE: usize = 16;

#[derive(Debug)]
pub(crate) struct Budget {
    remaining: Cell<usize>,
}

impl Budget {
    pub(crate) fn for_source(length: usize) -> Budget {
        Budget {
            remaining: Cell::new(FLOOR.saturating_add(length.saturating_mul(PER_SOURCE_BYTE))),
        }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.remaining.get()
    }

    pub(crate) fn spend(&self, bytes: usize) -> bool {
        let remaining = self.remaining.get();
        self.remaining.set(remaining.saturating_sub(bytes));
        bytes <= remaining
    }
}
