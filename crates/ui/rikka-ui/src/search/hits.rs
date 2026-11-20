use std::ops::Range;

use crate::search::query::Query;
use crate::search::state::Step;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub found: usize,
    pub current: Option<usize>,
}

impl Tally {
    #[must_use]
    pub fn new(found: usize, current: Option<usize>) -> Self {
        Self { found, current }
    }

    #[must_use]
    pub fn is_empty(self) -> bool {
        self.found == 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Backward,
}

#[must_use]
pub fn stepped(current: Option<usize>, count: usize, direction: Direction) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let last = count - 1;
    Some(match (current, direction) {
        (None, Direction::Forward) => 0,
        (None, Direction::Backward) => last,
        (Some(at), Direction::Forward) if at >= last => 0,
        (Some(at), Direction::Forward) => at + 1,
        (Some(0), Direction::Backward) => last,
        (Some(at), Direction::Backward) => at - 1,
    })
}

#[derive(Clone, Debug, Default)]
pub struct Hits {
    ranges: Vec<Range<usize>>,
    current: Option<usize>,
    searched: u64,
}

impl Hits {
    pub fn refresh(&mut self, query: &Query, text: &str) {
        query.find_all_into(text, &mut self.ranges);
        if self.searched != query.stamp() {
            self.searched = query.stamp();
            self.current = None;
        }
        self.current = self.current.filter(|at| *at < self.ranges.len());
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    #[must_use]
    pub fn current(&self) -> Option<usize> {
        self.current
    }

    #[must_use]
    pub fn current_range(&self) -> Option<Range<usize>> {
        self.ranges.get(self.current?).cloned()
    }

    #[must_use]
    pub fn tally(&self) -> Tally {
        Tally::new(self.ranges.len(), self.current)
    }

    pub fn forget(&mut self) {
        self.ranges.clear();
        self.current = None;
    }

    pub fn aim(&mut self, from: usize) -> Option<Range<usize>> {
        if self.ranges.is_empty() {
            self.current = None;
            return None;
        }
        let at = self
            .ranges
            .iter()
            .position(|range| range.end > from)
            .unwrap_or(0);
        self.current = Some(at);
        self.current_range()
    }

    pub fn step(&mut self, from: Range<usize>, forward: bool) -> Option<Range<usize>> {
        if self.ranges.is_empty() {
            self.current = None;
            return None;
        }
        let at = match forward {
            true => self
                .ranges
                .iter()
                .position(|range| range.start >= from.end)
                .unwrap_or(0),
            false => self
                .ranges
                .iter()
                .rposition(|range| range.end <= from.start)
                .unwrap_or(self.ranges.len() - 1),
        };
        self.current = Some(at);
        self.current_range()
    }

    pub fn walk(&mut self, direction: Direction) -> Option<Range<usize>> {
        self.current = stepped(self.current, self.ranges.len(), direction);
        self.current_range()
    }

    pub fn follow(&mut self, step: Option<Step>) -> Option<Range<usize>> {
        match step {
            Some(Step::Next) => self.walk(Direction::Forward),
            Some(Step::Previous) => self.walk(Direction::Backward),
            _ if self.current.is_some() => self.current_range(),
            _ => self.aim(0),
        }
    }
}
