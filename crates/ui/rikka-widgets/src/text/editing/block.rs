use std::ops::Range;
use std::sync::Arc;

use rikka_text::{Affinity, Align, Cursor, Family, FontStyle, TextSpan, Weight, Wrap};
use rikka_theme::{Palette, Syntax};
use rikka_types::Rect;

use rikka_ui::text::TextLayout;
use rikka_ui::text::{DisplayRevision, DisplayText, TextRun};

use crate::text::editing::rows::Rows;

const BREAK_WIDTH: f32 = 0.4;

pub(crate) const PIECE_LINES: usize = 16;

#[derive(Debug, Default)]
pub(crate) struct Block {
    look: Option<Look>,
    revision: Option<DisplayRevision>,
    pieces: Vec<Piece>,
}

#[derive(Debug)]
pub(crate) struct Piece {
    first: usize,
    lines: usize,
    run: TextRun,
    laid: Arc<TextLayout>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Look {
    pub(crate) width: Option<f32>,
    pub(crate) styling: Styling,
    pub(crate) palette: Palette,
    pub(crate) syntax: Syntax,
    pub(crate) coloured: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Styling {
    size: f32,
    line_height: f32,
    family: Family,
    weight: Weight,
    style: FontStyle,
    wrap: Wrap,
    align: Option<Align>,
    tab_width: u16,
}

pub(crate) struct Lines<'a> {
    pub(crate) laid: Range<usize>,
    pub(crate) retained: Range<usize>,
    pub(crate) template: &'a TextRun,
}

impl Block {
    pub(crate) fn sync(
        &mut self,
        shown: &DisplayText<'_>,
        lines: Lines<'_>,
        look: Look,
        mut spans: impl FnMut(&DisplayText<'_>, Range<usize>) -> Vec<TextSpan>,
    ) {
        if self.look.as_ref() != Some(&look) {
            self.pieces.clear();
            self.look = Some(look);
        }
        let width = self.look.as_ref().and_then(|look| look.width);
        let count = shown.line_count().max(1);
        let revision = shown.revision();
        if self.revision.replace(revision) != Some(revision) {
            self.pieces.retain(|piece| piece.first < count);
            for piece in &mut self.pieces {
                piece.refresh(shown, count, lines.template, width, &mut spans);
            }
        }
        let retained =
            piece_of(lines.retained.start)..piece_of(lines.retained.end.saturating_sub(1)) + 1;
        self.pieces
            .retain(|piece| retained.contains(&piece_of(piece.first)));
        let laid = piece_of(lines.laid.start)..piece_of(lines.laid.end.saturating_sub(1)) + 1;
        for index in laid {
            let first = index * PIECE_LINES;
            if first >= count {
                break;
            }
            let at = self.pieces.partition_point(|piece| piece.first < first);
            if self
                .pieces
                .get(at)
                .is_some_and(|piece| piece.first == first)
            {
                continue;
            }
            let piece = Piece::lay(shown, first, count, lines.template, width, &mut spans);
            self.pieces.insert(at, piece);
        }
    }

    pub(crate) fn pieces(&self) -> &[Piece] {
        &self.pieces
    }

    pub(crate) fn laid_lines(&self) -> Option<Range<usize>> {
        let first = self.pieces.first()?;
        let last = self.pieces.last()?;
        Some(first.first..last.first + last.lines)
    }

    pub(crate) fn widest(&self) -> f32 {
        self.pieces
            .iter()
            .map(|piece| piece.laid.widest())
            .fold(0.0, f32::max)
    }

    pub(crate) fn holding(&self, line: usize) -> Option<&Piece> {
        let at = self
            .pieces
            .partition_point(|piece| piece.first + piece.lines <= line);
        self.pieces.get(at).filter(|piece| piece.holds(line))
    }

    pub(crate) fn caret(
        &self,
        shown: &DisplayText<'_>,
        rows: &Rows,
        at: usize,
        affinity: Affinity,
    ) -> Option<(f32, f32)> {
        let piece = self.holding(shown.line_of(at))?;
        let (x, y) = piece.caret(shown, at, affinity)?;
        Some((x, y + piece.top(rows)))
    }

    pub(crate) fn cursor_at(
        &self,
        shown: &DisplayText<'_>,
        rows: &Rows,
        x: f32,
        y: f32,
    ) -> Option<(usize, Affinity)> {
        let line_height = self.pieces.first()?.run.line_height.max(1.0);
        let row = (y.max(0.0) / line_height).floor() as u32;
        let (line, _) = rows.at(row.min(rows.total().saturating_sub(1)));
        let at = self
            .pieces
            .partition_point(|piece| piece.first + piece.lines <= line);
        let piece = self.pieces.get(at).or_else(|| self.pieces.last())?;
        let top = piece.top(rows);
        Some(piece.cursor_at(shown, x, y - top))
    }

    pub(crate) fn selection(
        &self,
        shown: &DisplayText<'_>,
        rows: &Rows,
        range: Range<usize>,
    ) -> Vec<Rect> {
        let mut bands = Vec::new();
        for piece in &self.pieces {
            let top = piece.top(rows);
            bands.extend(
                piece
                    .selection(shown, range.clone())
                    .into_iter()
                    .map(|band| band.translate(rikka_types::Point::new(0.0, top))),
            );
        }
        bands
    }
}

impl Piece {
    fn lay(
        shown: &DisplayText<'_>,
        first: usize,
        count: usize,
        template: &TextRun,
        width: Option<f32>,
        spans: &mut impl FnMut(&DisplayText<'_>, Range<usize>) -> Vec<TextSpan>,
    ) -> Piece {
        let lines = PIECE_LINES.min(count - first);
        let run = run_of(shown, first..first + lines, template, spans);
        let laid = rikka_text::layout(run.as_shape(), width);
        Piece {
            first,
            lines,
            run,
            laid,
        }
    }

    fn refresh(
        &mut self,
        shown: &DisplayText<'_>,
        count: usize,
        template: &TextRun,
        width: Option<f32>,
        spans: &mut impl FnMut(&DisplayText<'_>, Range<usize>) -> Vec<TextSpan>,
    ) {
        let lines = PIECE_LINES.min(count - self.first);
        let run = run_of(shown, self.first..self.first + lines, template, spans);
        self.lines = lines;
        if run.content == self.run.content && run.spans == self.run.spans {
            return;
        }
        self.laid = rikka_text::layout(run.as_shape(), width);
        self.run = run;
    }

    pub(crate) fn first(&self) -> usize {
        self.first
    }

    pub(crate) fn lines(&self) -> Range<usize> {
        self.first..self.first + self.lines
    }

    pub(crate) fn holds(&self, line: usize) -> bool {
        self.lines().contains(&line)
    }

    pub(crate) fn run(&self) -> &TextRun {
        &self.run
    }

    pub(crate) fn laid(&self) -> &Arc<TextLayout> {
        &self.laid
    }

    pub(crate) fn rows(&self) -> usize {
        self.laid.row_count()
    }

    pub(crate) fn top(&self, rows: &Rows) -> f32 {
        rows.before(self.first) as f32 * self.run.line_height
    }

    pub(crate) fn drawn(&self, shown: &DisplayText<'_>) -> Range<usize> {
        shown.line_start(self.first)..shown.line_range(self.first + self.lines - 1).end
    }

    pub(crate) fn caret(
        &self,
        shown: &DisplayText<'_>,
        at: usize,
        affinity: Affinity,
    ) -> Option<(f32, f32)> {
        let cursor = self.cursor_of(shown, at)?;
        let caret = self.laid.caret(Cursor { affinity, ..cursor });
        Some((caret.x, caret.top))
    }

    fn cursor_at(&self, shown: &DisplayText<'_>, x: f32, y: f32) -> (usize, Affinity) {
        let bottom = (self.rows() as f32 * self.run.line_height - 1.0).max(0.0);
        let cursor = self.laid.hit(x.max(0.0), y.clamp(0.0, bottom));
        (self.offset_of(shown, cursor), cursor.affinity)
    }

    pub(crate) fn selection(&self, shown: &DisplayText<'_>, selection: Range<usize>) -> Vec<Rect> {
        let drawn = self.drawn(shown);
        let from = selection.start.clamp(drawn.start, drawn.end);
        let to = selection.end.clamp(drawn.start, drawn.end);
        let reaches_on = selection.end > drawn.end;
        if from >= to && !(reaches_on && selection.start <= drawn.end) {
            return Vec::new();
        }
        let (Some(from), Some(to)) = (self.cursor_of(shown, from), self.cursor_of(shown, to))
        else {
            return Vec::new();
        };
        let (from, to) = (self.laid.offset_of(from), self.laid.offset_of(to));
        let mut lit = self.laid.selection_rects(from..to).into_iter().peekable();
        let mut bands = Vec::new();
        let last = self.laid.row_count().saturating_sub(1);
        for row in self.laid.rows() {
            let ends_at = row.text_range().end;
            let carries_on =
                (ends_at < to || (reaches_on && row.index() == last)) && ends_at >= from;
            let trailing = match carries_on {
                true => self.run.size * BREAK_WIDTH,
                false => 0.0,
            };
            let first = bands.len();
            while let Some((_, band)) = lit.next_if(|(at, _)| *at == row.index()) {
                bands.push(band);
            }
            if let Some(last) = bands[first..]
                .iter_mut()
                .max_by(|one, other| one.max_x().total_cmp(&other.max_x()))
            {
                *last = Rect::new(last.x(), last.y(), last.width() + trailing, last.height());
            }
            if row.glyphs().is_empty() && carries_on {
                bands.push(Rect::new(0.0, row.top(), trailing, row.height()));
            }
        }
        bands
    }

    fn cursor_of(&self, shown: &DisplayText<'_>, at: usize) -> Option<Cursor> {
        let line = shown.line_of(at);
        let index = at.checked_sub(shown.line_start(line))?;
        line.checked_sub(self.first)
            .filter(|within| *within < self.lines)
            .map(|within| Cursor::new(within, index))
    }

    fn offset_of(&self, shown: &DisplayText<'_>, cursor: Cursor) -> usize {
        let line = self.first + cursor.line;
        shown.line_start(line) + cursor.index
    }
}

impl Look {
    pub(crate) fn new(
        template: &TextRun,
        width: Option<f32>,
        palette: Palette,
        syntax: Syntax,
        coloured: bool,
    ) -> Self {
        Look {
            width,
            styling: Styling::from_run(template),
            palette,
            syntax,
            coloured,
        }
    }
}

impl Styling {
    fn from_run(run: &TextRun) -> Self {
        Self {
            size: run.size,
            line_height: run.line_height,
            family: run.family.clone(),
            weight: run.weight,
            style: run.style,
            wrap: run.wrap,
            align: run.align,
            tab_width: run.tab_width,
        }
    }
}

fn piece_of(line: usize) -> usize {
    line / PIECE_LINES
}

fn run_of(
    shown: &DisplayText<'_>,
    lines: Range<usize>,
    template: &TextRun,
    spans: &mut impl FnMut(&DisplayText<'_>, Range<usize>) -> Vec<TextSpan>,
) -> TextRun {
    let origin = shown.line_start(lines.start);
    let end = shown.line_range(lines.end - 1).end;
    TextRun {
        content: shown.slice(origin..end).into(),
        spans: spans(shown, origin..end).into(),
        ..template.clone()
    }
}
