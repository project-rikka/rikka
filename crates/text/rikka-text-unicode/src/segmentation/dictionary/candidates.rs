use super::lexicon::{Lexicon, Matches};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Candidates {
    offset: usize,
    found: Matches,
    current: usize,
    marked: usize,
}

impl Default for Candidates {
    fn default() -> Self {
        Self {
            offset: usize::MAX,
            found: Matches::default(),
            current: 0,
            marked: 0,
        }
    }
}

impl Candidates {
    pub(crate) fn find(
        &mut self,
        lexicon: &Lexicon,
        text: &str,
        cursor: &mut usize,
        end: usize,
    ) -> usize {
        let start = *cursor;
        if start != self.offset {
            self.offset = start;
            lexicon.matches(text, start, end, &mut self.found);
        }
        let count = self.found.count;
        if count > 0 {
            *cursor = start + usize::from(self.found.lengths[count - 1]);
        }
        self.current = count.saturating_sub(1);
        self.marked = self.current;
        count
    }

    pub(crate) fn accept_marked(&self, cursor: &mut usize) -> usize {
        let length = usize::from(self.found.lengths[self.marked]);
        *cursor = self.offset + length;
        length
    }

    pub(crate) fn back_up(&mut self, cursor: &mut usize) -> bool {
        if self.current == 0 {
            return false;
        }
        self.current -= 1;
        *cursor = self.offset + usize::from(self.found.lengths[self.current]);
        true
    }

    pub(crate) fn longest_prefix(&self) -> usize {
        self.found.prefix
    }

    pub(crate) fn mark_current(&mut self) {
        self.marked = self.current;
    }

    pub(crate) fn marked_characters(&self) -> usize {
        usize::from(self.found.characters[self.marked])
    }
}
