use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use crate::search::query::Query;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
    pub at: usize,
    pub range: Range<usize>,
    pub current: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Found {
    query: Query,
    current: Option<usize>,
}

impl Found {
    #[must_use]
    pub fn new(query: Query) -> Self {
        Self {
            query,
            current: None,
        }
    }

    #[must_use]
    pub fn current(mut self, at: Option<usize>) -> Self {
        self.current = at;
        self
    }

    #[must_use]
    pub fn query(&self) -> &Query {
        &self.query
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.query.is_readable()
    }

    pub fn marks(&self, whole: &str, stamp: u64, window: Range<usize>) -> Vec<Mark> {
        if self.is_empty() || window.is_empty() {
            return Vec::new();
        }
        let ranges = ranges(&self.query, whole, stamp);
        ranges
            .iter()
            .enumerate()
            .filter_map(|(at, range)| {
                let start = range.start.max(window.start);
                let end = range.end.min(window.end);
                (start < end).then(|| Mark {
                    at,
                    range: start - window.start..end - window.start,
                    current: self.current == Some(at),
                })
            })
            .collect()
    }
}

type Read = (u64, u64, Rc<Vec<Range<usize>>>);

thread_local! {
    static READ: RefCell<Option<Read>> = const { RefCell::new(None) };
}

fn ranges(query: &Query, whole: &str, stamp: u64) -> Rc<Vec<Range<usize>>> {
    READ.with(|read| {
        let mut cached = read.borrow_mut();
        if let Some((was, searched, ranges)) = cached.as_ref()
            && *was == stamp
            && *searched == query.stamp()
        {
            return Rc::clone(ranges);
        }
        let ranges = Rc::new(query.find_all(whole));
        *cached = Some((stamp, query.stamp(), Rc::clone(&ranges)));
        ranges
    })
}
