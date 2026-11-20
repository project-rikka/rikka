use std::fmt::{self, Write};
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;

use rikka_text::TextSpan;
use rikka_text::{Align as TextAlign, Wrap};
use rikka_theme::Palette;
use rikka_types::{Color, Point, Rect};
use rustc_hash::FxHasher;

use crate::text::editing::EditingState;
use crate::text::editing::block::Piece;
use crate::text::editing::rows::Rows;
use crate::text::editor::{
    Edit, GUTTER_GAP, GUTTER_INSET, TEXT_INSET, TextEditor, TextEditorState,
};
use rikka_ui::text::{TextLayout, TextRun};
use rikka_ui::{PaintCtx, Quad, measure_text};

const SLACK: f32 = 1.0;
const EDIT_WIDTH: f32 = 3.0;
const EDIT_GONE: f32 = 2.0;

#[derive(Debug, Default)]
pub(crate) struct Digit {
    measured: Option<(TextRun, f32)>,
}

impl Digit {
    pub(crate) fn width(&mut self, template: &TextRun) -> f32 {
        if let Some((was, width)) = &self.measured
            && was == template
        {
            return *width;
        }
        let width = digit_width(template);
        self.measured = Some((template.clone(), width));
        width
    }
}

pub(crate) fn digit_width(template: &TextRun) -> f32 {
    let run = TextRun {
        content: "0".into(),
        ..template.clone()
    };
    measure_text(&run, None).width
}

#[derive(Debug, Default)]
pub(crate) struct Numbers {
    caches: Vec<GutterCache>,
}

#[derive(Debug)]
struct GutterCache {
    key: Key,
    template: TextRun,
    run: TextRun,
    laid: Arc<TextLayout>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Key {
    first: usize,
    lines: usize,
    rows: u64,
    here: Option<usize>,
    width: f32,
    bright: Color,
}

impl Numbers {
    fn sync(
        &mut self,
        editing: &EditingState,
        template: &TextRun,
        width: f32,
        bright: Color,
        caret_line: usize,
    ) {
        let pieces = editing.block.pieces();
        self.caches.retain(|numbered| {
            pieces
                .binary_search_by_key(&numbered.key.first, Piece::first)
                .is_ok()
        });
        for piece in pieces {
            let lines = piece.lines();
            let key = Key {
                first: lines.start,
                lines: lines.len(),
                rows: rows_digest(&editing.rows, lines.clone()),
                here: lines.contains(&caret_line).then_some(caret_line),
                width,
                bright,
            };
            let at = self
                .caches
                .partition_point(|numbered| numbered.key.first < key.first);
            match self.caches.get_mut(at) {
                Some(numbered) if numbered.key.first == key.first => {
                    if numbered.key != key || numbered.template != *template {
                        *numbered = GutterCache::build(key, template, &editing.rows);
                    }
                }
                _ => self
                    .caches
                    .insert(at, GutterCache::build(key, template, &editing.rows)),
            }
        }
    }

    fn cached(&self, first: usize) -> Option<&GutterCache> {
        self.caches
            .binary_search_by_key(&first, |numbered| numbered.key.first)
            .ok()
            .map(|at| &self.caches[at])
    }
}

impl GutterCache {
    fn build(key: Key, template: &TextRun, rows: &Rows) -> GutterCache {
        let mut numbers = String::new();
        let here =
            write_numbers(&mut numbers, &key, rows).expect("writing to a String cannot fail");
        let spans: Vec<TextSpan> = here
            .map(|at| {
                let end = at + numbers[at..].find('\n').unwrap_or(numbers.len() - at);
                vec![TextSpan::new(at..end).color(key.bright)]
            })
            .unwrap_or_default();
        let run = TextRun {
            content: numbers.into(),
            align: Some(TextAlign::Right),
            wrap: Wrap::None,
            spans: spans.into(),
            ..template.clone()
        };
        let laid = rikka_text::layout_approx(run.as_shape(), Some(key.width));
        GutterCache {
            key,
            template: template.clone(),
            run,
            laid,
        }
    }
}

fn rows_digest(rows: &Rows, lines: Range<usize>) -> u64 {
    let mut hasher = FxHasher::default();
    for line in lines {
        rows.row_count(line).hash(&mut hasher);
    }
    hasher.finish()
}

impl TextEditor {
    pub(crate) fn draw_gutter(
        &self,
        state: &mut TextEditorState,
        ctx: &mut PaintCtx<'_>,
        gutter: f32,
        palette: Palette,
        template: &TextRun,
    ) {
        if !self.gutter && self.edits.is_empty() {
            return;
        }
        let editing = &state.editing;
        let frame = editing.frame;
        let room = frame.room;
        let width = gutter - GUTTER_INSET - GUTTER_GAP;
        if self.gutter {
            let caret_line = self.editing.document.content().caret_line_column().0;
            state
                .numbers
                .sync(editing, template, width, palette.text, caret_line);
        }
        let strip = match self.gutter {
            true => GUTTER_GAP,
            false => TEXT_INSET,
        };
        let marks = room.x() + gutter - strip + (strip - EDIT_WIDTH) / 2.0;
        let count = self.editing.document.line_count();
        let numbers = &state.numbers;
        ctx.clipped(room, |ctx| {
            ctx.scrolled(Point::new(0.0, frame.scrolled.y), |ctx| {
                for piece in editing.block.pieces() {
                    let top = frame.sheet_top + piece.top(&editing.rows);
                    ctx.part(("gutter", piece.first()), |ctx| {
                        if let Some(numbered) =
                            numbers.cached(piece.first()).filter(|_| self.gutter)
                        {
                            let height = piece.rows() as f32 * numbered.run.line_height + SLACK;
                            ctx.draw_laid_text(
                                Rect::new(room.x() + GUTTER_INSET, top, width, height),
                                numbered.run.clone(),
                                numbered.laid.clone(),
                                palette.text_muted,
                            );
                        }
                        let lines = piece.lines();
                        let reach = match lines.end >= count {
                            true => lines.start..lines.end + 1,
                            false => lines,
                        };
                        for (line, edit) in self.edits.in_lines(reach) {
                            let y = frame.sheet_top
                                + editing.rows.before(*line) as f32 * frame.line_height;
                            let bar = match edit {
                                Edit::Gone => Rect::new(
                                    marks - EDIT_WIDTH / 2.0,
                                    y - EDIT_GONE / 2.0,
                                    EDIT_WIDTH * 2.0,
                                    EDIT_GONE,
                                ),
                                _ => Rect::new(
                                    marks,
                                    y,
                                    EDIT_WIDTH,
                                    editing.rows.row_count(*line) as f32 * frame.line_height,
                                ),
                            };
                            ctx.draw_quad(
                                Quad::new(bar)
                                    .fill(edit.color(palette))
                                    .radius(EDIT_WIDTH / 2.0),
                            );
                        }
                    });
                }
            });
        });
    }
}

fn write_numbers(out: &mut String, key: &Key, rows: &Rows) -> Result<Option<usize>, fmt::Error> {
    let mut here = None;
    for line in key.first..key.first + key.lines {
        if line != key.first {
            out.write_char('\n')?;
        }
        if key.here == Some(line) {
            here = Some(out.len());
        }
        write!(out, "{}", line + 1)?;
        for _ in 1..rows.row_count(line) {
            out.write_char('\n')?;
        }
    }
    Ok(here)
}
