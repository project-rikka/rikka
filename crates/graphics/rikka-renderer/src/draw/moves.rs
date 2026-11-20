use crate::draw::arena::Ledger;

#[derive(Debug)]
pub(crate) struct Moves {
    pub(crate) offsets: Vec<[f32; 2]>,
    pub(crate) ledger: Ledger,
}

impl Default for Moves {
    fn default() -> Self {
        Self {
            offsets: vec![[0.0; 2]],
            ledger: Ledger::default(),
        }
    }
}

impl Moves {
    pub(crate) fn set(&mut self, place: u32, offset: [f32; 2]) {
        let at = place as usize;
        if self.offsets.len() <= at {
            let from = self.offsets.len();
            self.offsets.resize(at + 1, [0.0; 2]);
            self.ledger.note(from as u32, (at + 1 - from) as u32);
        }
        if self.offsets[at] != offset {
            self.offsets[at] = offset;
            self.ledger.note(place, 1);
        }
    }
}
