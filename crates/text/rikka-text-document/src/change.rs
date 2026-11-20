use std::collections::VecDeque;

use crate::rope::Rope;

const REMEMBERED: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub row: usize,
    pub column: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Splice {
    pub start: usize,
    pub old_end: usize,
    pub new_end: usize,
    pub start_place: Place,
    pub old_end_place: Place,
    pub new_end_place: Place,
}

#[derive(Debug, Default)]
pub(crate) struct Changes {
    remembered: VecDeque<(u64, Splice)>,
    lost: u64,
}

impl Changes {
    pub(crate) fn record(&mut self, revision: u64, splice: Splice) {
        if self.remembered.len() == REMEMBERED
            && let Some((dropped, _)) = self.remembered.pop_front()
        {
            self.lost = self.lost.max(dropped);
        }
        self.remembered.push_back((revision, splice));
    }

    pub(crate) fn forget(&mut self) {
        self.remembered.clear();
    }

    pub(crate) fn since(
        &self,
        known: u64,
        now: u64,
    ) -> Option<impl Iterator<Item = Splice> + use<'_>> {
        if known < self.lost {
            return None;
        }
        if known != now {
            let (oldest, _) = self.remembered.front()?;
            if *oldest > known + 1 {
                return None;
            }
        }
        Some(
            self.remembered
                .iter()
                .filter(move |(at, _)| *at > known)
                .map(|(_, splice)| *splice),
        )
    }
}

pub fn place_of(rope: &Rope, byte: usize) -> Place {
    let byte = byte.min(rope.len());
    let row = rope.line_of(byte);
    Place {
        row,
        column: byte - rope.line_start(row),
    }
}
