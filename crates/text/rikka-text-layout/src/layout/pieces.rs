use std::ops::Range;

use crate::layout::plan::{Piece, Source};
use crate::layout::tabs::{tab_advance, tab_stop};
use crate::shaping::{Context, GlyphColumns, Run, RunRequest, Shaped, flag, shape_run};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TabAdvance {
    pub(crate) piece: u32,
    pub(crate) glyph: u32,
    pub(crate) advance: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Measured {
    pub(crate) content: f32,
    pub(crate) hanging: f32,
}

pub(crate) struct Pieces<'a> {
    pub(crate) shaped: &'a Shaped,
    pub(crate) font_size: f32,
    pub(crate) letter_spacing: f32,
    pub(crate) fragments: &'a mut GlyphColumns,
    pub(crate) pieces: &'a mut Vec<Piece>,
    pub(crate) tabs: &'a mut Vec<TabAdvance>,
}

impl Pieces<'_> {
    pub(crate) fn collect(&mut self, row: Range<u32>, content_end: u32) -> Range<u32> {
        let first = self.pieces.len();
        let shaped = self.shaped;
        let mut index = shaped.run_at_offset(row.start);
        while index < shaped.runs.len() && shaped.runs[index].text.start < row.end {
            self.collect_run(&shaped.runs[index], row.clone());
            index += 1;
        }
        self.split_hanging(first, content_end);
        if shaped.bidi {
            self.isolate_separators(first);
        }
        first as u32..self.pieces.len() as u32
    }

    fn collect_run(&mut self, run: &Run, row: Range<u32>) {
        let shaped = self.shaped;
        let glyphs = &shaped.glyphs;
        let start = row.start.max(run.text.start);
        let end = row.end.min(run.text.end);
        if start >= end {
            return;
        }
        let first = glyphs.first_at_or_after(run.glyph_range(), start);
        let last = glyphs.first_at_or_after(run.glyph_range(), end);
        if first >= last {
            return;
        }
        let cut_start = start == row.start && start > 0 && !glyphs.safe_before(first);
        let cut_end = end == row.end && (end as usize) < shaped.text.len() && {
            let after = glyphs.first_at_or_after(0..glyphs.len(), end);
            after < glyphs.len() && !glyphs.safe_before(after)
        };
        let offset = |index: usize| match index {
            index if index >= last => end,
            index if index == first => start,
            index => glyphs.clusters[index],
        };
        let safe_from = if cut_start {
            next_safe(glyphs, first + 1, last)
        } else {
            first
        };
        if cut_start && safe_from >= last {
            let after = !cut_end && end < row.end;
            self.fragment(
                run,
                start..end,
                Context {
                    before: false,
                    after,
                },
            );
            return;
        }
        if cut_start {
            let context = Context {
                before: false,
                after: true,
            };
            self.fragment(run, start..offset(safe_from), context);
        }
        let safe_to = if cut_end {
            previous_safe(glyphs, safe_from, last)
        } else {
            last
        };
        if safe_from < safe_to {
            self.push(run, Source::Shaped, safe_from..safe_to);
        }
        if cut_end {
            let from = offset(safe_to);
            let context = Context {
                before: from > row.start,
                after: false,
            };
            self.fragment(run, from..end, context);
        }
    }

    fn fragment(&mut self, run: &Run, range: Range<u32>, context: Context) {
        let shaped = self.shaped;
        let before = self.fragments.len();
        shape_run(
            &RunRequest {
                text: &shaped.text,
                range: range.start as usize..range.end as usize,
                face: &shaped.faces[usize::from(run.face)],
                script: run.script,
                rtl: run.level.is_rtl(),
                context,
                caps: shaped.caps,
            },
            self.fragments,
        );
        let after = self.fragments.len();
        self.push(run, Source::Fragment, before..after);
    }

    fn push(&mut self, run: &Run, source: Source, glyphs: Range<usize>) {
        self.pieces.push(Piece {
            source,
            glyphs: glyphs.start as u32..glyphs.end as u32,
            face: run.face,
            style: run.style,
            level: run.level,
            script: run.script,
            hanging: false,
            width: 0.0,
        });
    }

    fn split_hanging(&mut self, first: usize, content_end: u32) {
        let shaped = self.shaped;
        let base = shaped.base_level;
        let mut index = first;
        while index < self.pieces.len() {
            let piece = &self.pieces[index];
            let columns = piece.columns(shaped, self.fragments);
            let at = columns.first_at_or_after(piece.glyph_range(), content_end);
            if at < piece.glyphs.end as usize {
                if at > piece.glyphs.start as usize {
                    let mut tail = piece.clone();
                    tail.glyphs.start = at as u32;
                    self.pieces[index].glyphs.end = at as u32;
                    self.pieces.insert(index + 1, tail);
                    index += 1;
                }
                for piece in &mut self.pieces[index..] {
                    piece.hanging = true;
                    piece.level = base;
                }
                return;
            }
            index += 1;
        }
    }

    fn isolate_separators(&mut self, first: usize) {
        let shaped = self.shaped;
        let base = shaped.base_level;
        let mut index = first;
        while index < self.pieces.len() {
            let piece = &self.pieces[index];
            if piece.hanging || piece.level == base {
                index += 1;
                continue;
            }
            let columns = piece.columns(shaped, self.fragments);
            let range = piece.glyph_range();
            let Some(separator) = range
                .clone()
                .find(|&glyph| columns.flags[glyph] & flag::SEPARATOR != 0)
            else {
                index += 1;
                continue;
            };
            let mut from = separator;
            while from > range.start && columns.flags[from - 1] & flag::WHITESPACE != 0 {
                from -= 1;
            }
            let mut middle = piece.clone();
            middle.glyphs = from as u32..separator as u32 + 1;
            middle.level = base;
            let mut tail = piece.clone();
            tail.glyphs.start = separator as u32 + 1;
            self.pieces[index].glyphs.end = from as u32;
            self.pieces.insert(index + 1, middle);
            self.pieces.insert(index + 2, tail);
            index += 2;
        }
        let pieces = &mut *self.pieces;
        let mut len = first;
        for index in first..pieces.len() {
            if pieces[index].glyphs.start < pieces[index].glyphs.end {
                pieces.swap(len, index);
                len += 1;
            }
        }
        pieces.truncate(len);
    }

    pub(crate) fn measure(&mut self, pieces: Range<u32>) -> Measured {
        let shaped = self.shaped;
        let mut measured = Measured::default();
        let mut x = 0.0;
        for index in pieces.start as usize..pieces.end as usize {
            let piece = &self.pieces[index];
            let hanging = piece.hanging;
            let columns = piece.columns(shaped, self.fragments);
            let size = shaped.styles[usize::from(piece.style)].size(self.font_size);
            let mut width = 0.0;
            if shaped.tabs {
                let stop = tab_stop(
                    &shaped.faces[usize::from(piece.face)],
                    size,
                    shaped.tab_width,
                );
                for glyph in piece.glyph_range() {
                    let advance = if columns.flags[glyph] & flag::TAB != 0 {
                        let advance = tab_advance(x + width, stop);
                        self.tabs.push(TabAdvance {
                            piece: index as u32,
                            glyph: glyph as u32,
                            advance,
                        });
                        advance
                    } else {
                        columns.advances[glyph] * size
                    };
                    width += advance + self.spacing_after(columns, glyph);
                }
            } else {
                let range = piece.glyph_range();
                width = columns.advances[range.clone()].iter().sum::<f32>() * size;
                if self.letter_spacing != 0.0 {
                    let spaced = range
                        .filter(|&glyph| columns.ends_spaced_cluster(glyph))
                        .count();
                    width += spaced as f32 * self.letter_spacing;
                }
            }
            x += width;
            self.pieces[index].width = width;
            if hanging {
                measured.hanging += width
            } else {
                measured.content += width
            }
        }
        measured
    }

    #[inline]
    fn spacing_after(&self, columns: &GlyphColumns, glyph: usize) -> f32 {
        if columns.ends_spaced_cluster(glyph) {
            self.letter_spacing
        } else {
            0.0
        }
    }
}

fn is_safe_boundary(glyphs: &GlyphColumns, index: usize) -> bool {
    glyphs.clusters[index] != glyphs.clusters[index - 1] && glyphs.safe_before(index)
}

fn next_safe(glyphs: &GlyphColumns, from: usize, last: usize) -> usize {
    (from..last)
        .find(|&index| is_safe_boundary(glyphs, index))
        .unwrap_or(last)
}

fn previous_safe(glyphs: &GlyphColumns, floor: usize, last: usize) -> usize {
    (floor + 1..last)
        .rev()
        .find(|&index| is_safe_boundary(glyphs, index))
        .unwrap_or(floor)
}
