use std::time::{Duration, Instant};

use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Boundary, Column, Direction, Line, Point as GridPoint};
use alacritty_terminal::term::Term;
use alacritty_terminal::term::search::{Match, RegexSearch};
use rikka_ui::search::{Query, Tally};

const MAX_MATCHES: usize = 10_000;

const SEARCH_BUDGET: Duration = Duration::from_millis(2);

const SEARCH_MARGIN: i32 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Read {
    query: u64,
    generation: u64,
    lines: usize,
    columns: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MarkedView {
    read: Read,
    offset: usize,
}

#[derive(Clone, Copy, Debug)]
struct MatchCount {
    from: GridPoint,
    count: usize,
    current: Option<usize>,
    done: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Found {
    dfas: Option<(u64, RegexSearch)>,
    read: Option<Read>,
    marked: Option<MarkedView>,
    counting: Option<MatchCount>,
    count: usize,
    current: Option<usize>,
    current_match: Option<Match>,
    screen: Vec<Match>,
}

impl Found {
    pub(crate) fn forget(&mut self) {
        self.dfas = None;
        self.read = None;
        self.marked = None;
        self.counting = None;
        self.count = 0;
        self.current = None;
        self.current_match = None;
        self.screen.clear();
    }

    pub(crate) fn on_screen(&self) -> impl Iterator<Item = (bool, &Match)> {
        let current_match = self.current_match.clone();
        self.screen
            .iter()
            .map(move |found| (current_match.as_ref() == Some(found), found))
    }

    pub(crate) fn tally(&self) -> Tally {
        Tally::new(self.count, self.current)
    }

    pub(crate) fn is_counting(&self) -> bool {
        self.counting.is_some_and(|counting| !counting.done)
    }

    pub(crate) fn read<T: EventListener>(
        &mut self,
        term: &Term<T>,
        query: &Query,
        generation: u64,
    ) {
        let Some(expression) = query.expression() else {
            self.forget();
            return;
        };
        if self.dfas.as_ref().map(|(stamp, _)| *stamp) != Some(query.stamp()) {
            self.dfas = RegexSearch::new(expression)
                .ok()
                .map(|dfas| (query.stamp(), dfas));
            self.counting = None;
        }
        if self.dfas.is_none() {
            self.forget();
            return;
        }

        let read = Read {
            query: query.stamp(),
            generation,
            lines: term.total_lines(),
            columns: term.columns(),
        };
        if self.read != Some(read) {
            self.read = Some(read);
            self.counting = Some(MatchCount {
                from: GridPoint::new(term.topmost_line(), Column(0)),
                count: 0,
                current: None,
                done: false,
            });
        }
        self.mark(term);
        self.count(term);
    }

    fn mark<T: EventListener>(&mut self, term: &Term<T>) {
        let (Some(read), Some((_, dfas))) = (self.read, self.dfas.as_mut()) else {
            return;
        };
        let offset = term.grid().display_offset();
        let marked = MarkedView { read, offset };
        if self.marked == Some(marked) {
            return;
        }
        self.marked = Some(marked);
        self.screen.clear();
        let lines = term.screen_lines();
        let top = term.bottommost_line() - offset as i32 - (lines as i32 - 1);
        let first = GridPoint::new((top - SEARCH_MARGIN).max(term.topmost_line()), Column(0));
        let last = GridPoint::new(term.bottommost_line() - offset as i32, term.last_column());
        let mut origin = first;
        while let Some(hit) = term.regex_search_right(dfas, origin, last) {
            let end = *hit.end();
            if shown(&hit, offset, lines) {
                self.screen.push(hit);
            }
            if end >= last {
                break;
            }
            let next = end.add(term, Boundary::Grid, 1);
            if next <= origin {
                break;
            }
            origin = next;
        }
    }

    fn count<T: EventListener>(&mut self, term: &Term<T>) {
        let (Some((_, dfas)), Some(counting)) = (self.dfas.as_mut(), self.counting.as_mut()) else {
            return;
        };
        if counting.done {
            return;
        }
        let last = GridPoint::new(term.bottommost_line(), term.last_column());
        let until = Instant::now() + SEARCH_BUDGET;
        loop {
            let Some(hit) = term.regex_search_right(dfas, counting.from, last) else {
                counting.done = true;
                break;
            };
            if self.current_match.as_ref() == Some(&hit) {
                counting.current = Some(counting.count);
            }
            counting.count += 1;
            let end = *hit.end();
            if counting.count >= MAX_MATCHES || end >= last {
                counting.done = true;
                break;
            }
            let next = end.add(term, Boundary::Grid, 1);
            if next <= counting.from {
                counting.done = true;
                break;
            }
            counting.from = next;
            if Instant::now() >= until {
                break;
            }
        }
        self.count = counting.count;
        if counting.done {
            self.current = counting.current;
        }
    }

    pub(crate) fn step<T: EventListener>(&mut self, term: &mut Term<T>, direction: Direction) {
        let top = GridPoint::new(term.topmost_line(), Column(0));
        let bottom = GridPoint::new(term.bottommost_line(), term.last_column());
        let from = self.from(term, direction);
        let next = self.next_index(direction);
        let Some((_, dfas)) = self.dfas.as_mut() else {
            return;
        };

        let found = match direction {
            Direction::Right => term
                .regex_search_right(dfas, from, bottom)
                .or_else(|| term.regex_search_right(dfas, top, bottom)),
            Direction::Left => term
                .regex_search_left(dfas, from, top)
                .or_else(|| term.regex_search_left(dfas, bottom, top)),
        };
        let Some(found) = found else {
            self.current_match = None;
            self.current = None;
            return;
        };

        self.current = next;
        term.scroll_to_point(*found.start());
        self.current_match = Some(found);
        self.mark(term);
    }

    fn next_index(&self, direction: Direction) -> Option<usize> {
        if self.is_counting() || self.count == 0 {
            return None;
        }
        let direction = match direction {
            Direction::Right => rikka_ui::search::Direction::Forward,
            Direction::Left => rikka_ui::search::Direction::Backward,
        };
        rikka_ui::search::stepped(self.current, self.count, direction)
    }

    fn from<T: EventListener>(&self, term: &Term<T>, direction: Direction) -> GridPoint {
        if let Some(current_match) = &self.current_match {
            let at = match direction {
                Direction::Right => *current_match.end(),
                Direction::Left => *current_match.start(),
            };
            let boundary = Boundary::Grid;
            return match direction {
                Direction::Right => at.add(term, boundary, 1),
                Direction::Left => at.sub(term, boundary, 1),
            };
        }
        let offset = term.grid().display_offset() as i32;
        let top = term.bottommost_line() - offset - (term.screen_lines() as i32 - 1);
        match direction {
            Direction::Right => GridPoint::new(top.max(term.topmost_line()), Column(0)),
            Direction::Left => GridPoint::new(term.bottommost_line() - offset, term.last_column()),
        }
    }
}

fn shown(found: &Match, display_offset: usize, lines: usize) -> bool {
    let top = Line(-(display_offset as i32));
    let bottom = top + lines.saturating_sub(1);
    found.end().line >= top && found.start().line <= bottom
}
