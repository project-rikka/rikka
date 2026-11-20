use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Granularity {
    Character,
    Word,
    Line,
}

impl Granularity {
    pub fn from_click_count(presses: u32) -> Self {
        match presses {
            0 | 1 => Self::Character,
            2 => Self::Word,
            _ => Self::Line,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Endpoints {
    pub anchor: usize,
    pub focus: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionOrigin {
    granularity: Granularity,
    start: usize,
    end: usize,
}

impl SelectionOrigin {
    pub fn new(granularity: Granularity, range: Range<usize>) -> Self {
        Self {
            granularity,
            start: range.start,
            end: range.end,
        }
    }

    pub fn granularity(&self) -> Granularity {
        self.granularity
    }

    pub fn extend(&self, unit: Range<usize>) -> Endpoints {
        if unit.start < self.start {
            Endpoints {
                anchor: self.end,
                focus: unit.start,
            }
        } else if unit.end > self.end {
            Endpoints {
                anchor: self.start,
                focus: unit.end,
            }
        } else {
            Endpoints {
                anchor: self.start,
                focus: self.end,
            }
        }
    }
}
