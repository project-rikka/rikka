use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use rikka_text_layout::{Align, Caret, ParagraphLayout, Wrap};
use rikka_types::{Rect, Size};
use smallvec::SmallVec;

use crate::cursor::Cursor;
use crate::layout_row::LayoutRow;
use crate::rows::{LineStart, Rows};
use crate::shaped::Shaped;
use crate::shaping::LayoutRequest;
use crate::text::Text;

pub(crate) const SLACK: f32 = 1.0;

#[repr(align(64))]
pub struct TextLayout {
    shaped: Arc<Shaped>,
    request: LayoutRequest,
    lines: SmallVec<[ParagraphLayout; 1]>,
    starts: SmallVec<[LineStart; 1]>,
    rows: usize,
    widest: f32,
    elided: Option<Text>,
    steady: bool,
    bytes: usize,
}

impl TextLayout {
    pub(crate) fn new(shaped: Arc<Shaped>, request: LayoutRequest) -> TextLayout {
        let plain = request.params(false);
        let elided_at = request
            .width
            .filter(|width| request.ellipsize && width.is_finite());
        let mut lines = SmallVec::with_capacity(shaped.line_count());
        let mut starts = SmallVec::with_capacity(shaped.line_count());
        let mut rows = 0;
        let mut widest: f32 = 0.0;
        let mut bytes = 0;
        let mut elided = false;
        for line in shaped.lines() {
            let paragraph = &line.shaped.paragraph;
            let mut layout = paragraph.layout(&plain);
            if let Some(width) = elided_at
                && layout.width().ceil() > width + SLACK
            {
                paragraph.layout_into(&request.params(true), &mut layout);
                elided |= layout.is_elided();
            }
            starts.push(LineStart {
                row: rows as u32,
                text: line.range.start,
            });
            rows += layout.rows().len();
            widest = widest.max(layout.width());
            bytes += layout.heap_bytes();
            lines.push(layout);
        }
        let elided = elided.then(|| visible_text(&shaped, &lines));
        let steady = !shaped.is_rtl()
            && elided.is_none()
            && matches!(request.align, None | Some(Align::Left));
        TextLayout {
            bytes: bytes + lines.len() * size_of::<ParagraphLayout>(),
            shaped,
            request,
            lines,
            starts,
            rows,
            widest,
            elided,
            steady,
        }
    }

    #[must_use]
    pub fn serves_width(&self, width: f32) -> bool {
        self.serves(Some(width), false)
    }

    pub(crate) fn serves(&self, width: Option<f32>, tolerant: bool) -> bool {
        let bits = |width: Option<f32>| width.map(f32::to_bits);
        if bits(self.request.width) == bits(width) {
            return true;
        }
        let requested = width.unwrap_or(f32::INFINITY);
        let request_width = self.request.width.unwrap_or(f32::INFINITY);
        if tolerant && (requested - request_width).abs() < 1.0 {
            return true;
        }
        if !self.steady {
            return false;
        }
        match self.request.wrap {
            Wrap::None => !self.request.ellipsize || self.widest.ceil() <= requested + SLACK,
            _ => self.widest <= requested && requested <= request_width,
        }
    }

    pub(crate) fn request(&self) -> &LayoutRequest {
        &self.request
    }

    pub(crate) fn bytes(&self) -> usize {
        self.bytes
    }

    #[must_use]
    pub fn shaped(&self) -> &Arc<Shaped> {
        &self.shaped
    }

    #[must_use]
    pub fn text(&self) -> &Text {
        self.shaped.text()
    }

    #[must_use]
    pub fn visible_text(&self) -> &Text {
        self.elided.as_ref().unwrap_or(self.shaped.text())
    }

    #[must_use]
    pub fn is_elided(&self) -> bool {
        self.elided.is_some()
    }

    #[must_use]
    pub fn font_size(&self) -> f32 {
        self.request.font_size
    }

    #[must_use]
    pub fn line_height(&self) -> f32 {
        self.request.line_height
    }

    #[must_use]
    pub fn letter_spacing(&self) -> f32 {
        self.request.letter_spacing
    }

    #[must_use]
    pub fn width(&self) -> Option<f32> {
        self.request.width
    }

    #[must_use]
    pub fn row_count(&self) -> usize {
        self.rows
    }

    #[must_use]
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    #[must_use]
    pub fn widest(&self) -> f32 {
        self.widest
    }

    #[must_use]
    pub fn min_content_width(&self) -> f32 {
        if self.request.wrap == Wrap::None && !self.is_elided() {
            return self.widest;
        }
        let params = self.request.params(false);
        self.shaped
            .lines()
            .iter()
            .map(|line| line.shaped.paragraph.min_content_width(&params))
            .fold(0.0, f32::max)
    }

    #[must_use]
    pub fn height(&self) -> f32 {
        self.rows.max(1) as f32 * self.request.line_height
    }

    #[must_use]
    pub fn size(&self) -> Size {
        Size::new(self.widest.ceil(), self.height())
    }

    pub fn rows(&self) -> impl Iterator<Item = LayoutRow<'_>> {
        self.rows_from(0)
    }

    pub fn rows_between(&self, top: f32, bottom: f32) -> impl Iterator<Item = LayoutRow<'_>> {
        let first = (top / self.request.line_height).floor().max(0.0) as usize;
        self.rows_from(first.saturating_sub(1)).between(top, bottom)
    }

    fn rows_from(&self, row: usize) -> Rows<'_> {
        Rows::new(
            &self.lines,
            &self.starts,
            self.request.line_height,
            row.min(self.rows),
        )
    }

    #[must_use]
    pub fn cursor_of(&self, at: usize) -> Cursor {
        let lines = self.shaped.lines();
        let line = lines
            .partition_point(|line| line.range.start <= at)
            .saturating_sub(1);
        let current = &lines[line];
        Cursor::new(line, at.min(current.range.end) - current.range.start)
    }

    #[must_use]
    pub fn offset_of(&self, cursor: Cursor) -> usize {
        let lines = self.shaped.lines();
        let Some(line) = lines.get(cursor.line) else {
            return self.text().len();
        };
        (line.range.start + cursor.index).min(line.range.end)
    }

    #[must_use]
    pub fn hit(&self, x: f32, y: f32) -> Cursor {
        let last = self.lines.len() - 1;
        if y < 0.0 {
            return Cursor::new(0, 0);
        }
        let row = (y / self.request.line_height).floor() as usize;
        if row >= self.rows {
            let range = &self.shaped.lines()[last].range;
            return Cursor::before(last, range.len());
        }
        let line = self
            .starts
            .partition_point(|start| start.row as usize <= row)
            .saturating_sub(1);
        let within = y - self.top_of(line);
        let hit = self.lines[line].hit(x, within);
        Cursor {
            line,
            index: hit.index,
            affinity: hit.affinity,
        }
    }

    #[must_use]
    pub fn caret(&self, cursor: Cursor) -> Caret {
        let line = cursor.line.min(self.lines.len() - 1);
        let caret = self.lines[line].caret(cursor.within_line());
        Caret {
            row: caret.row + self.starts[line].row as usize,
            top: caret.top + self.top_of(line),
            ..caret
        }
    }

    #[must_use]
    pub fn selection_rects(&self, range: Range<usize>) -> Vec<(usize, Rect)> {
        let mut bands = Vec::new();
        if range.start >= range.end {
            return bands;
        }
        let mut rects = Vec::new();
        for (index, line) in self.shaped.lines().iter().enumerate() {
            if line.range.start > range.end || range.start > line.range.end {
                continue;
            }
            let local = |at: usize| at.clamp(line.range.start, line.range.end) - line.range.start;
            rects.clear();
            self.lines[index].selection_rects(local(range.start)..local(range.end), &mut rects);
            let first = self.starts[index].row as usize;
            let top = self.top_of(index);
            bands.extend(rects.iter().map(|rect| {
                (
                    first + rect.row,
                    Rect::new(rect.x, rect.y + top, rect.width, rect.height),
                )
            }));
        }
        bands
    }

    #[must_use]
    pub fn paragraph(&self, line: usize) -> Option<&ParagraphLayout> {
        self.lines.get(line)
    }

    fn top_of(&self, line: usize) -> f32 {
        self.starts[line].row as f32 * self.request.line_height
    }
}

fn visible_text(shaped: &Shaped, lines: &[ParagraphLayout]) -> Text {
    let text = shaped.text();
    let mut visible_text = String::with_capacity(text.len() + 3);
    for (line, layout) in shaped.lines().iter().zip(lines) {
        visible_text.push_str(&layout.visible_text());
        visible_text.push_str(&text[line.range.end..line.next]);
    }
    Text::from(visible_text)
}

impl PartialEq for TextLayout {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.shaped, &other.shaped) && self.request == other.request
    }
}

impl fmt::Debug for TextLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextLayout")
            .field("text", self.visible_text())
            .field("width", &self.request.width)
            .field("rows", &self.rows)
            .finish_non_exhaustive()
    }
}
