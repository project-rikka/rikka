const INLINE_CAPACITY: usize = 32;

#[derive(Debug)]
pub(crate) struct Marks {
    inline: [(u8, char); INLINE_CAPACITY],
    len: usize,
    spilled: Vec<(u8, char)>,
}

impl Marks {
    pub(crate) fn new() -> Self {
        Self {
            inline: [(0, '\0'); INLINE_CAPACITY],
            len: 0,
            spilled: Vec::new(),
        }
    }

    #[inline]
    pub(crate) fn push(&mut self, class: u8, character: char) {
        if self.len < INLINE_CAPACITY {
            self.inline[self.len] = (class, character);
        } else {
            if self.len == INLINE_CAPACITY {
                self.spilled.clear();
                self.spilled.extend_from_slice(&self.inline);
            }
            self.spilled.push((class, character));
        }
        self.len += 1;
    }

    #[inline]
    pub(crate) fn last_class(&self) -> Option<u8> {
        self.as_slice().last().map(|&(class, _)| class)
    }

    #[inline]
    pub(crate) fn as_slice(&self) -> &[(u8, char)] {
        if self.len <= INLINE_CAPACITY {
            &self.inline[..self.len]
        } else {
            &self.spilled
        }
    }

    pub(crate) fn sort_canonically(&mut self) {
        if self.len > INLINE_CAPACITY {
            self.spilled.sort_by_key(|&(class, _)| class);
            return;
        }
        let marks = &mut self.inline[..self.len];
        for index in 1..marks.len() {
            let mark = marks[index];
            let mut position = index;
            while position > 0 && marks[position - 1].0 > mark.0 {
                marks[position] = marks[position - 1];
                position -= 1;
            }
            marks[position] = mark;
        }
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        self.len = 0;
    }
}
