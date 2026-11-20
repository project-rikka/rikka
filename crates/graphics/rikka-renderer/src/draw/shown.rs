use crate::draw::arena::Ledger;

#[derive(Debug)]
pub(crate) struct VisibilityTable {
    pub(crate) words: Vec<u32>,
    pub(crate) ledger: Ledger,
}

impl Default for VisibilityTable {
    fn default() -> Self {
        Self {
            words: vec![1],
            ledger: Ledger::default(),
        }
    }
}

impl VisibilityTable {
    pub(crate) fn place(index: u32) -> u32 {
        index + 1
    }

    pub(crate) fn set(&mut self, place: u32, shown: bool) {
        let (word, bit) = ((place / 32) as usize, 1u32 << (place % 32));
        if self.words.len() <= word {
            let from = self.words.len();
            self.words.resize(word + 1, 0);
            self.ledger.note(from as u32, (word + 1 - from) as u32);
        }
        let was = self.words[word];
        let now = match shown {
            true => was | bit,
            false => was & !bit,
        };
        if now != was {
            self.words[word] = now;
            self.ledger.note(word as u32, 1);
        }
    }
}
