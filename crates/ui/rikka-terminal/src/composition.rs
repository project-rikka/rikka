use std::ops::Range;

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Point as GridPoint};
use alacritty_terminal::term::cell::{Cell, Flags};
use unicode_width::UnicodeWidthChar;

use crate::layout::Layout;

#[derive(Debug, Default)]
pub(crate) struct Composition {
    text: String,
    cursor: Option<Range<usize>>,
}

impl Composition {
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub(crate) fn set(&mut self, text: &str, cursor: Option<Range<usize>>) -> bool {
        let cursor = cursor.map(|cursor| boundary(text, cursor.start)..boundary(text, cursor.end));
        if self.text == text && self.cursor == cursor {
            return false;
        }
        self.text.clear();
        self.text.push_str(text);
        self.cursor = cursor;
        true
    }

    pub(crate) fn clear(&mut self) {
        self.set("", None);
    }

    #[must_use]
    pub(crate) fn place(&self, at: GridPoint<usize>, layout: Layout) -> PreeditLayout {
        let cells = self.cells();
        let cursor = self
            .cursor
            .clone()
            .unwrap_or(self.text.len()..self.text.len());
        let caret = cells_of(&self.text[..cursor.start]);
        let converting = caret..caret + cells_of(&self.text[cursor.start..cursor.end]);
        let rows = wrap(&cells, at, layout);
        let caret = caret_of(&rows, caret, at, layout.columns());

        PreeditLayout {
            cells,
            rows,
            converting,
            caret,
        }
    }

    fn cells(&self) -> Vec<Cell> {
        let mut cells: Vec<Cell> = Vec::new();
        for character in self.text.chars() {
            match character.width().unwrap_or(0) {
                0 => {
                    if let Some(last) = cells.last_mut() {
                        last.push_zerowidth(character);
                    }
                }
                2 => {
                    cells.push(cell(character, Flags::WIDE_CHAR));
                    cells.push(cell(' ', Flags::WIDE_CHAR_SPACER));
                }
                _ => cells.push(cell(character, Flags::empty())),
            }
        }
        cells
    }
}

#[derive(Debug)]
pub(crate) struct PreeditLayout {
    cells: Vec<Cell>,
    rows: Vec<Row>,
    converting: Range<usize>,
    caret: GridPoint<usize>,
}

impl PreeditLayout {
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    #[must_use]
    pub(crate) fn caret(&self) -> GridPoint<usize> {
        self.caret
    }

    pub(crate) fn segments(&self) -> impl Iterator<Item = Segment<'_>> {
        self.rows.iter().map(|row| Segment {
            line: row.line,
            column: row.column,
            cells: &self.cells[row.start..row.end],
            converting: self.converting.start.clamp(row.start, row.end) - row.start
                ..self.converting.end.clamp(row.start, row.end) - row.start,
        })
    }
}

#[derive(Debug)]
pub(crate) struct Segment<'a> {
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) cells: &'a [Cell],
    pub(crate) converting: Range<usize>,
}

#[derive(Clone, Copy, Debug)]
struct Row {
    line: usize,
    column: usize,
    start: usize,
    end: usize,
}

fn cell(character: char, flags: Flags) -> Cell {
    Cell {
        c: character,
        flags,
        ..Cell::default()
    }
}

fn wrap(cells: &[Cell], at: GridPoint<usize>, layout: Layout) -> Vec<Row> {
    let columns = layout.columns();
    let mut rows: Vec<Row> = Vec::new();
    let mut line = at.line;
    let mut column = at.column.0.min(columns);
    let mut start = 0;

    while start < cells.len() {
        let mut take = (columns - column).min(cells.len() - start);
        if take < cells.len() - start && cells[start + take - 1].flags.contains(Flags::WIDE_CHAR) {
            take -= 1;
        }
        if take == 0 {
            if column == 0 {
                start += 2;
            } else {
                line += 1;
                column = 0;
            }
            continue;
        }
        rows.push(Row {
            line,
            column,
            start,
            end: start + take,
        });
        start += take;
        line += 1;
        column = 0;
    }

    let last = rows.last().map_or(0, |row| row.line + 1);
    let shift = last.saturating_sub(layout.screen_lines()).min(at.line);
    for row in &mut rows {
        row.line -= shift;
    }
    rows.retain(|row| row.line < layout.screen_lines());
    rows
}

fn cells_of(text: &str) -> usize {
    text.chars()
        .map(|character| character.width().unwrap_or(0))
        .sum()
}

fn caret_of(rows: &[Row], caret: usize, at: GridPoint<usize>, columns: usize) -> GridPoint<usize> {
    if let Some(row) = rows
        .iter()
        .find(|row| (row.start..row.end).contains(&caret))
    {
        return GridPoint::new(row.line, Column(row.column + caret - row.start));
    }
    match rows.last() {
        Some(row) if caret >= row.end => GridPoint::new(
            row.line,
            Column((row.column + row.end - row.start).min(columns.saturating_sub(1))),
        ),
        _ => at,
    }
}

fn boundary(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while at > 0 && !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}
