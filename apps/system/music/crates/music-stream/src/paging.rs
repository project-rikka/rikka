#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OffsetPaging {
    size: usize,
    fetched: usize,
    finished: bool,
}

impl OffsetPaging {
    #[must_use]
    pub fn new(size: usize) -> OffsetPaging {
        OffsetPaging {
            size: size.max(1),
            fetched: 0,
            finished: false,
        }
    }

    #[must_use]
    pub fn size(&self) -> usize {
        self.size
    }

    #[must_use]
    pub fn next_offset(&self) -> Option<usize> {
        (!self.finished).then_some(self.fetched)
    }

    pub fn arrived(&mut self, count: usize, total: Option<usize>) {
        self.fetched += count;
        self.finished = count == 0
            || match total {
                Some(total) => self.fetched >= total,
                None => count < self.size,
            };
    }
}
