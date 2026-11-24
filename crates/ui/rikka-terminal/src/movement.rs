use alacritty_terminal::Term;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Boundary, Column, Line, Point};
use alacritty_terminal::term::cell::{Cell, Flags};
use rikka_types::Key;

use crate::words;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Movement {
    Left,
    Right,
    Up,
    Down,
    WordLeft,
    WordRight,
    LineStart,
    LineEnd,
    PageUp,
    PageDown,
    Top,
    Bottom,
}

impl Movement {
    pub(crate) fn from_key(key: Key, control: bool) -> Option<Self> {
        Some(match (key, control) {
            (Key::ArrowLeft, false) => Self::Left,
            (Key::ArrowRight, false) => Self::Right,
            (Key::ArrowLeft, true) => Self::WordLeft,
            (Key::ArrowRight, true) => Self::WordRight,
            (Key::ArrowUp, false) => Self::Up,
            (Key::ArrowDown, false) => Self::Down,
            (Key::Home, false) => Self::LineStart,
            (Key::End, false) => Self::LineEnd,
            (Key::Home, true) => Self::Top,
            (Key::End, true) => Self::Bottom,
            (Key::PageUp, false) => Self::PageUp,
            (Key::PageDown, false) => Self::PageDown,
            _ => return None,
        })
    }

    pub(crate) fn apply<T>(self, term: &Term<T>, point: Point) -> Point {
        let page = term.screen_lines() as i32;
        match self {
            Self::Left => point.sub(term, Boundary::Grid, 1),
            Self::Right => point.add(term, Boundary::Grid, 1),
            Self::Up => vertical(term, point, -1),
            Self::Down => vertical(term, point, 1),
            Self::PageUp => vertical(term, point, -page),
            Self::PageDown => vertical(term, point, page),
            Self::WordLeft => word_start(term, point),
            Self::WordRight => word_end(term, point),
            Self::LineStart => Point::new(point.line, Column(0)),
            Self::LineEnd => line_end(term, point.line),
            Self::Top => first_cell(term),
            Self::Bottom => last_cell(term),
        }
    }
}

pub(crate) fn first_cell<T>(term: &Term<T>) -> Point {
    Point::new(term.topmost_line(), Column(0))
}

pub(crate) fn last_cell<T>(term: &Term<T>) -> Point {
    Point::new(term.bottommost_line(), term.last_column())
}

fn vertical<T>(term: &Term<T>, point: Point, lines: i32) -> Point {
    let line = (point.line.0 + lines).clamp(term.topmost_line().0, term.bottommost_line().0);
    Point::new(Line(line), point.column)
}

fn word_end<T>(term: &Term<T>, point: Point) -> Point {
    let last = last_cell(term);
    let mut next = point;
    loop {
        if next == last {
            return last;
        }
        next = next.add(term, Boundary::Grid, 1);
        if !is_blank(term, next) {
            return *words::word_cells(term, next).end();
        }
    }
}

fn word_start<T>(term: &Term<T>, point: Point) -> Point {
    let first = first_cell(term);
    let mut previous = point;
    loop {
        if previous == first {
            return first;
        }
        previous = previous.sub(term, Boundary::Grid, 1);
        if !is_blank(term, previous) {
            return *words::word_cells(term, previous).start();
        }
    }
}

fn line_end<T>(term: &Term<T>, line: Line) -> Point {
    let row = &term.grid()[line];
    let filled = (0..term.columns())
        .rev()
        .find(|&column| !is_blank_cell(&row[Column(column)]))
        .unwrap_or(0);
    Point::new(line, Column(filled))
}

fn is_blank<T>(term: &Term<T>, point: Point) -> bool {
    is_blank_cell(&term.grid()[point])
}

fn is_blank_cell(cell: &Cell) -> bool {
    cell.c.is_whitespace() && !cell.flags.contains(Flags::WIDE_CHAR_SPACER)
}
