use std::ops::Range;

use crate::compute::lane::{Marks, reached};

#[derive(Debug)]
pub(crate) struct Slots {
    free: Vec<Range<u32>>,
    pending: Vec<(Marks, Range<u32>)>,
}

impl Slots {
    pub(crate) fn new(capacity: u32) -> Slots {
        Slots {
            free: vec![Range {
                start: 0,
                end: capacity,
            }],
            pending: Vec::new(),
        }
    }

    pub(crate) fn take(&mut self, count: u32) -> Option<u32> {
        let found = self
            .free
            .iter()
            .position(|range| range.end - range.start >= count)?;
        let range = &mut self.free[found];
        let start = range.start;
        range.start += count;
        if range.start == range.end {
            self.free.remove(found);
        }
        Some(start)
    }

    pub(crate) fn release(&mut self, range: Range<u32>, after: Marks) {
        self.pending.push((after, range));
    }

    pub(crate) fn collect(&mut self, completed: Marks) {
        let mut returned = Vec::new();
        self.pending.retain(|(after, range)| {
            let done = reached(*after, completed);
            if done {
                returned.push(range.clone());
            }
            !done
        });
        for range in returned {
            self.recycle(range);
        }
    }

    fn recycle(&mut self, range: Range<u32>) {
        let at = self.free.partition_point(|free| free.start < range.start);
        self.free.insert(at, range);
        if at + 1 < self.free.len() && self.free[at].end == self.free[at + 1].start {
            let next = self.free.remove(at + 1);
            self.free[at].end = next.end;
        }
        if at > 0 && self.free[at - 1].end == self.free[at].start {
            let this = self.free.remove(at);
            self.free[at - 1].end = this.end;
        }
    }
}
