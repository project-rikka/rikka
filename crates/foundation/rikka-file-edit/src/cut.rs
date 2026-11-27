use std::ops::Range;

#[derive(Clone, Debug)]
pub(crate) struct Cut {
    pub(crate) at: Range<usize>,
    pub(crate) added: usize,
}

impl Cut {
    pub(crate) fn moved(&self, offset: usize) -> Option<usize> {
        match offset {
            _ if offset < self.at.start => Some(offset),
            _ if offset < self.at.end => None,
            _ => Some(offset - self.at.len() + self.added),
        }
    }

    pub(crate) fn moved_line(&self, line: &Range<usize>) -> Option<Range<usize>> {
        if self.at.start <= line.end && line.start < self.at.end {
            return None;
        }
        Some(self.moved(line.start)?..self.moved(line.end)?)
    }
}
