use rikka_text_layout::Affinity;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Cursor {
    pub line: usize,
    pub index: usize,
    pub affinity: Affinity,
}

impl Cursor {
    #[must_use]
    pub const fn new(line: usize, index: usize) -> Self {
        Self {
            line,
            index,
            affinity: Affinity::After,
        }
    }

    #[must_use]
    pub const fn before(line: usize, index: usize) -> Self {
        Self {
            line,
            index,
            affinity: Affinity::Before,
        }
    }

    pub(crate) fn within_line(self) -> rikka_text_layout::Cursor {
        rikka_text_layout::Cursor::new(self.index, self.affinity)
    }
}
