use std::ops::Range;

#[derive(Debug, Default)]
pub(crate) struct WordRanges {
    free: Vec<Range<u32>>,
    capacity: u32,
}

impl WordRanges {
    pub(crate) fn capacity(&self) -> u32 {
        self.capacity
    }

    pub(crate) fn allocate(&mut self, len: u32) -> Option<Range<u32>> {
        let at = self
            .free
            .iter()
            .position(|range| range.len() >= len as usize)?;
        let range = &mut self.free[at];
        let taken = range.start..range.start + len;
        range.start += len;
        if range.start == range.end {
            self.free.remove(at);
        }
        Some(taken)
    }

    pub(crate) fn release(&mut self, range: Range<u32>) -> u32 {
        if range.is_empty() {
            return 0;
        }
        let at = self.free.partition_point(|free| free.start < range.start);
        let joins_before = at > 0 && self.free[at - 1].end == range.start;
        let joins_after = at < self.free.len() && self.free[at].start == range.end;
        let span = match (joins_before, joins_after) {
            (true, true) => {
                self.free[at - 1].end = self.free[at].end;
                self.free.remove(at);
                &self.free[at - 1]
            }
            (true, false) => {
                self.free[at - 1].end = range.end;
                &self.free[at - 1]
            }
            (false, true) => {
                self.free[at].start = range.start;
                &self.free[at]
            }
            (false, false) => {
                self.free.insert(at, range);
                &self.free[at]
            }
        };
        span.end - span.start
    }

    pub(crate) fn grow(&mut self, capacity: u32) {
        let old = std::mem::replace(&mut self.capacity, capacity);
        self.release(old..capacity);
    }

    pub(crate) fn used_end(&self) -> u32 {
        match self.free.last() {
            Some(last) if last.end == self.capacity => last.start,
            _ => self.capacity,
        }
    }
}
