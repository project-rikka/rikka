use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use rikka_markdown::markdown_text;
use rikka_thread::{Round, Turn};
use rikka_ui::search::{Action, Direction, Found, Query, Search, Step, Tally, stepped};

use crate::transcript::Blocks;

#[derive(Clone, Debug)]
pub(crate) enum Searched {
    Turn(Arc<Turn>),
    Round(Arc<Turn>, usize),
    Arriving(Arc<Round>),
}

impl Searched {
    pub(crate) fn read<R>(&self, with: impl FnOnce(&str) -> R) -> R {
        match self {
            Searched::Turn(turn) => with(turn.body().text()),
            Searched::Round(turn, round) => {
                let body = turn.body();
                let text = body
                    .rounds()
                    .get(*round)
                    .map(Round::text)
                    .unwrap_or_default();
                with(text)
            }
            Searched::Arriving(round) => with(round.text()),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Says {
    Nothing,
    Plain(Searched),
    Marked(Searched),
}

#[derive(Clone, Debug)]
pub(crate) struct Block {
    pub(crate) stamp: u64,
    pub(crate) says: Says,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Counted {
    stamp: u64,
    count: usize,
}

#[derive(Debug)]
pub(crate) struct Counting {
    search: SearchQuery,
    blocks: Vec<Counted>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SearchQuery {
    query: u64,
    blocks: u64,
}

#[derive(Debug)]
pub(crate) struct Recount {
    search: SearchQuery,
    query: Query,
    counts: Vec<Counted>,
    says: Vec<Block>,
}

impl Recount {
    #[must_use]
    pub(crate) fn count(self) -> Counting {
        Finding::count(self.search, &self.query, &self.says, &self.counts)
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Marks {
    searched: u64,
    standing: Option<(usize, usize)>,
    found: Found,
}

impl Marks {
    pub(crate) fn at(&self, block: usize) -> (u64, Found) {
        let current = self
            .standing
            .filter(|(at, _)| *at == block)
            .map(|(_, at)| at);
        let mut hasher = DefaultHasher::new();
        self.searched.hash(&mut hasher);
        current.hash(&mut hasher);
        (hasher.finish(), self.found.clone().current(current))
    }
}

#[derive(Debug, Default)]
pub struct Finding {
    search: Search,
    blocks: Vec<Counted>,
    current: Option<usize>,
    counted: SearchQuery,
    counting: Option<SearchQuery>,
}

impl Finding {
    pub fn search(&self) -> &Search {
        &self.search
    }

    pub fn is_open(&self) -> bool {
        self.search.is_open()
    }

    pub(crate) fn is_idle(&self) -> bool {
        !self.is_open()
            && self.blocks.is_empty()
            && self.current.is_none()
            && self.counted == SearchQuery::default()
            && self.counting.is_none()
    }

    pub(crate) fn open(&mut self, seed: Option<&str>) {
        self.search.open(seed);
    }

    pub(crate) fn act(&mut self, action: Action) {
        self.search.act(action);
    }

    pub(crate) fn wants(&mut self, blocks: u64) -> Option<SearchQuery> {
        let query = self.search.query();
        if !self.search.is_open() || !query.is_readable() {
            self.forget();
            return None;
        }
        let search = SearchQuery {
            query: query.stamp(),
            blocks,
        };
        if self.counted == search || self.counting.is_some() {
            self.step();
            return None;
        }
        if self.counted.query != search.query {
            self.blocks.clear();
            self.current = None;
        }
        self.counting = Some(search);
        self.step();
        Some(search)
    }

    pub(crate) fn count(
        search: SearchQuery,
        query: &Query,
        blocks: &[Block],
        counts: &[Counted],
    ) -> Counting {
        let counted = blocks
            .iter()
            .enumerate()
            .map(|(at, block)| {
                let standing = counts.get(at).filter(|counts| counts.stamp == block.stamp);
                if let Some(counts) = standing {
                    return counts.clone();
                }
                let count = match &block.says {
                    Says::Nothing => 0,
                    Says::Plain(searched) => searched.read(|text| query.count(text)),
                    Says::Marked(searched) => {
                        let plain = searched.read(markdown_text);
                        query.count(&plain)
                    }
                };
                Counted {
                    stamp: block.stamp,
                    count,
                }
            })
            .collect();
        Counting {
            search,
            blocks: counted,
        }
    }

    pub(crate) fn counted(&mut self, counting: &Counting) {
        if self.counting != Some(counting.search) {
            return;
        }
        self.counting = None;
        self.counted = counting.search;
        self.blocks.clone_from(&counting.blocks);
        self.step();
    }

    pub(crate) fn counts(&self) -> Vec<Counted> {
        self.blocks.clone()
    }

    pub(crate) fn query(&self) -> Query {
        self.search.query()
    }

    pub(crate) fn recount(&mut self, blocks: &Blocks) -> Option<Recount> {
        let search = self.wants(blocks.revision())?;
        Some(Recount {
            search,
            query: self.query(),
            counts: self.counts(),
            says: blocks.says(),
        })
    }

    pub(crate) fn marks(&self) -> Marks {
        Marks {
            searched: self.search.stamp(),
            standing: self.standing(),
            found: self.search.found(),
        }
    }

    pub(crate) fn reveals(&self) -> Option<usize> {
        self.standing().map(|(block, _)| block)
    }

    fn forget(&mut self) {
        self.blocks.clear();
        self.current = None;
        self.counted = SearchQuery::default();
        self.counting = None;
        self.search.report(Tally::default());
    }

    fn step(&mut self) {
        let found = self.found();
        self.current = match self.search.take_step() {
            Some(Step::Next) => stepped(self.current, found, Direction::Forward),
            Some(Step::Previous) => stepped(self.current, found, Direction::Backward),
            _ => self
                .current
                .filter(|at| *at < found)
                .or((found > 0).then_some(0)),
        };
        self.search.report(Tally::new(found, self.current));
    }

    fn found(&self) -> usize {
        self.blocks.iter().map(|block| block.count).sum()
    }

    fn standing(&self) -> Option<(usize, usize)> {
        let mut left = self.current?;
        for (block, counted) in self.blocks.iter().enumerate() {
            if left < counted.count {
                return Some((block, left));
            }
            left -= counted.count;
        }
        None
    }
}
