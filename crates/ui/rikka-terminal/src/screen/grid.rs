use std::ops::RangeInclusive;
use std::rc::Rc;

use alacritty_terminal::Term;
use alacritty_terminal::grid::{Dimensions, Row as GridRow};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::selection::SelectionRange;
use alacritty_terminal::term::TermDamage;
use alacritty_terminal::term::cell::Cell;
use alacritty_terminal::term::color::{COUNT, Colors};

use crate::screen::layer::{LayerCell, Sketch};
use crate::screen::lines::LinesCell;
use crate::screen::row::{CellStyle, Row, Span};
use crate::screen::{Frame, paint_row};
use crate::session::Proxy;

const REBASE: u32 = 1 << 12;

#[derive(Default)]
pub(crate) struct Grid {
    lines: Vec<Shown>,
    strip: Rc<LinesCell>,
    previous: Vec<Option<Shown>>,
    placed: Vec<Option<Placed>>,
    claimed: Vec<bool>,
    shifts: Vec<i32>,
    selection: Option<SelectionRange>,
    colors: Option<Colors>,
    stale: bool,
    keys: u64,
    sketch: Sketch,
}

#[derive(Clone, Copy)]
struct Placed {
    from: usize,
    dirty: bool,
}

pub(crate) struct Shown {
    key: u64,
    layer: Rc<LayerCell>,
    cells: Vec<Cell>,
    band: Option<Span>,
    origin: usize,
    dirty: bool,
}

struct Shot<'a> {
    row: &'a GridRow<Cell>,
    at: Line,
    columns: usize,
    selection: Option<SelectionRange>,
}

impl Shot<'_> {
    fn cells(&self) -> &[Cell] {
        &self.row[..Column(self.columns)]
    }

    fn origin(&self) -> usize {
        self.cells().as_ptr().addr()
    }

    fn band(&self) -> Option<Span> {
        let range = self.selection?;
        if self.at < range.start.line || self.at > range.end.line {
            return None;
        }
        let start = match range.is_block || self.at == range.start.line {
            true => range.start.column.0,
            false => 0,
        };
        let end = match range.is_block || self.at == range.end.line {
            true => range.end.column.0 + 1,
            false => self.columns,
        };
        let end = end.min(self.columns);
        (start < end).then(|| Span::from(start..end))
    }
}

impl Shown {
    pub(crate) fn key(&self) -> u64 {
        self.key
    }

    pub(crate) fn layer(&self) -> &Rc<LayerCell> {
        &self.layer
    }

    fn shows(&self, shot: &Shot<'_>) -> bool {
        self.band == shot.band() && self.cells.as_slice() == shot.cells()
    }

    fn take(&mut self, shot: &Shot<'_>) {
        self.origin = shot.origin();
        self.band = shot.band();
        self.cells.clear();
        self.cells.extend_from_slice(shot.cells());
    }
}

impl Grid {
    pub(crate) fn lines(&self) -> &[Shown] {
        &self.lines
    }

    pub(crate) fn strip(&self) -> &Rc<LinesCell> {
        &self.strip
    }

    pub(crate) fn invalidate(&mut self) {
        self.stale = true;
    }

    pub(crate) fn regrow(&mut self, count: usize) -> bool {
        if self.lines.len() == count {
            return false;
        }
        self.lines.truncate(count);
        while self.lines.len() < count {
            self.keys += 1;
            let layer = Rc::<LayerCell>::default();
            layer.place(self.strip.top() + self.lines.len() as i32);
            self.lines.push(Shown {
                key: self.keys,
                layer,
                cells: Vec::new(),
                band: None,
                origin: 0,
                dirty: true,
            });
        }
        true
    }

    pub(crate) fn copy(&mut self, term: &mut Term<Proxy>) -> bool {
        let regrown = self.regrow(term.screen_lines());
        if self
            .colors
            .is_none_or(|colors| (0..COUNT).any(|index| colors[index] != term.colors()[index]))
        {
            self.colors = Some(*term.colors());
            self.stale = true;
        }
        let stale = std::mem::take(&mut self.stale);
        for shown in &mut self.lines {
            shown.dirty |= stale;
        }
        let full = match term.damage() {
            TermDamage::Full => true,
            TermDamage::Partial(lines) => {
                for damaged in lines {
                    if let Some(shown) = self.lines.get_mut(damaged.line) {
                        shown.dirty = true;
                    }
                }
                false
            }
        };
        term.reset_damage();

        let count = self.lines.len();
        let display_offset = term.grid().display_offset();
        let selection = term
            .selection
            .as_ref()
            .and_then(|selection| selection.to_range(term));
        if selection != self.selection {
            for range in [self.selection, selection].into_iter().flatten() {
                for line in shown_lines(range, display_offset, count) {
                    if let Some(shown) = self.lines.get_mut(line) {
                        shown.dirty = true;
                    }
                }
            }
            self.selection = selection;
        }

        let columns = term.columns();
        let shot = |line: usize| {
            let at = Line(line as i32) - display_offset;
            Shot {
                row: &term.grid()[at],
                at,
                columns,
                selection,
            }
        };
        let shift = match full && !stale {
            true => self.rearrange(&shot),
            false => 0,
        };
        for (line, shown) in self.lines.iter_mut().enumerate() {
            if shown.dirty {
                shown.take(&shot(line));
            }
        }
        self.settle(shift);
        regrown
    }

    fn rearrange<'a>(&mut self, shot: &impl Fn(usize) -> Shot<'a>) -> i32 {
        let count = self.lines.len();
        self.placed.clear();
        self.placed.resize(count, None);
        self.claimed.clear();
        self.claimed.resize(count, false);
        for line in 0..count {
            let shot = shot(line);
            let origin = shot.origin();
            let from = match self.lines[line].origin == origin {
                true => Some(line),
                false => (0..count)
                    .find(|&from| !self.claimed[from] && self.lines[from].origin == origin),
            };
            if let Some(from) = from.filter(|&from| self.lines[from].shows(&shot)) {
                self.place(line, from, false);
            }
        }
        for line in 0..count {
            if self.placed[line].is_none() && !self.claimed[line] {
                self.place(line, line, true);
            }
        }
        let mut spare = 0;
        for line in 0..count {
            if self.placed[line].is_some() {
                continue;
            }
            while self.claimed[spare] {
                spare += 1;
            }
            self.place(line, spare, true);
        }

        self.previous.clear();
        self.previous.extend(self.lines.drain(..).map(Some));
        self.shifts.clear();
        for (line, placed) in self.placed.iter().enumerate() {
            let Placed { from, dirty } = placed.expect("every line placed");
            if !dirty {
                self.shifts.push(from as i32 - line as i32);
            }
            let mut shown = self.previous[from].take().expect("each line placed once");
            shown.dirty = dirty;
            self.lines.push(shown);
        }
        self.shifts.sort_unstable();
        most_common(&self.shifts).unwrap_or(0)
    }

    fn place(&mut self, line: usize, from: usize, dirty: bool) {
        self.placed[line] = Some(Placed { from, dirty });
        self.claimed[from] = true;
    }

    fn settle(&mut self, shift: i32) {
        let top = match self.strip.top() + shift {
            top if top.unsigned_abs() > REBASE => 0,
            top => top,
        };
        self.strip.scroll_to(top);
        for (line, shown) in self.lines.iter().enumerate() {
            shown.layer.place(top + line as i32);
        }
    }

    pub(crate) fn sketch(&mut self, frame: &Frame<'_>, colors: &Colors, text: &mut String) {
        let mut row = Row::default();
        for shown in &mut self.lines {
            if !shown.dirty {
                continue;
            }
            shown.dirty = false;
            row.clear();
            for (column, cell) in shown.cells.iter().enumerate() {
                let selected = shown
                    .band
                    .is_some_and(|band| band.start <= column && column < band.end);
                row.push(CellStyle::new(cell, selected, frame.palette, colors));
            }
            paint_row(&mut self.sketch, frame, 0, &row, text);
            shown.layer.show(&mut self.sketch);
        }
    }
}

impl std::fmt::Debug for Grid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Grid")
            .field("lines", &self.lines.len())
            .field("strip", &self.strip)
            .field("selection", &self.selection)
            .finish_non_exhaustive()
    }
}

fn most_common(sorted: &[i32]) -> Option<i32> {
    sorted
        .chunk_by(|a, b| a == b)
        .max_by_key(|run| run.len())
        .map(|run| run[0])
}

fn shown_lines(
    range: SelectionRange,
    display_offset: usize,
    count: usize,
) -> RangeInclusive<usize> {
    let last = count.saturating_sub(1);
    let shown = |line: Line| ((line.0 + display_offset as i32).max(0) as usize).min(last);
    shown(range.start.line)..=shown(range.end.line)
}
