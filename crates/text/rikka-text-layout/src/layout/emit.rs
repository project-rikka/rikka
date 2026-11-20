use rikka_text_unicode::{Level, visual_to_logical};
use smallvec::SmallVec;

use crate::fonts::EmMetrics;
use crate::layout::decoration::{Decoration, DecorationKind};
use crate::layout::glyph::Glyph;
use crate::layout::params::{Align, LayoutParams};
use crate::layout::pieces::TabAdvance;
use crate::layout::plan::{Piece, RowPlan};
use crate::layout::row::Row;
use crate::shaping::{GlyphColumns, Shaped, flag};

const DECORATION_JOIN: f32 = 1e-3;

pub(crate) struct RowOutput<'a> {
    pub(crate) rows: &'a mut SmallVec<[Row; 1]>,
    pub(crate) glyphs: &'a mut Vec<Glyph>,
    pub(crate) decorations: &'a mut Vec<Decoration>,
}

pub(crate) struct Emitter<'a> {
    pub(crate) shaped: &'a Shaped,
    pub(crate) params: &'a LayoutParams,
    pub(crate) pieces: &'a [Piece],
    pub(crate) fragments: &'a GlyphColumns,
    pub(crate) tabs: &'a [TabAdvance],
    pub(crate) levels: &'a mut Vec<Level>,
    pub(crate) order: &'a mut Vec<usize>,
    pub(crate) align_width: f32,
}

#[derive(Clone, Copy, Debug)]
struct Placement {
    left: f32,
    origin: f32,
    extra: f32,
    width: f32,
}

impl Emitter<'_> {
    pub(crate) fn emit_row(
        &mut self,
        index: usize,
        plan: &RowPlan,
        last: bool,
        out: &mut RowOutput<'_>,
    ) {
        let shaped = self.shaped;
        let font_size = self.params.font_size;
        let line_height = self.params.line_height;
        let pieces = &self.pieces[plan.pieces.start as usize..plan.pieces.end as usize];
        self.visual_order(pieces);
        let (ascent, descent) = self.extents(pieces);
        let top = index as f32 * line_height;
        let baseline = top + (line_height - (ascent + descent)) / 2.0 + ascent;
        let placement = self.placement(plan, last);
        let glyphs_start = out.glyphs.len();
        let decorations_start = out.decorations.len();
        let mut pen = placement.origin;
        for position in 0..pieces.len() {
            let logical = if shaped.bidi {
                self.order[position]
            } else {
                position
            };
            let piece = &pieces[logical];
            let piece_index = plan.pieces.start as usize + logical;
            let size = shaped.styles[usize::from(piece.style)].size(font_size);
            let from = pen;
            let extra = if piece.hanging { 0.0 } else { placement.extra };
            pen = self.emit_piece(piece, piece_index, size, extra, pen, out.glyphs);
            if !piece.hanging {
                self.decorate(piece, size, from..pen, decorations_start, out.decorations);
            }
        }
        out.rows.push(Row {
            top,
            height: line_height,
            baseline,
            left: placement.left,
            width: placement.width,
            origin: placement.origin,
            ascent,
            descent,
            text: plan.start..plan.end,
            glyphs: glyphs_start as u32..out.glyphs.len() as u32,
            decorations: decorations_start as u32..out.decorations.len() as u32,
            rtl: shaped.base_level.is_rtl(),
            ellipsis: plan.ellipsis,
        });
    }

    fn visual_order(&mut self, pieces: &[Piece]) {
        if !self.shaped.bidi {
            return;
        }
        self.levels.clear();
        self.levels.extend(pieces.iter().map(|piece| piece.level));
        visual_to_logical(self.levels, self.order);
    }

    fn extents(&self, pieces: &[Piece]) -> (f32, f32) {
        let shaped = self.shaped;
        let font_size = self.params.font_size;
        if pieces.is_empty() {
            let metrics = shaped.base_metrics();
            return (metrics.ascent * font_size, metrics.descent * font_size);
        }
        pieces
            .iter()
            .fold((0.0_f32, 0.0_f32), |(ascent, descent), piece| {
                let metrics = shaped.strut_metrics(piece.style, piece.face);
                let size = shaped.styles[usize::from(piece.style)].size(font_size);
                (
                    ascent.max(metrics.ascent * size),
                    descent.max(metrics.descent * size),
                )
            })
    }

    fn placement(&self, plan: &RowPlan, last: bool) -> Placement {
        let rtl = self.shaped.base_level.is_rtl();
        let width = plan.content_width;
        let free = self.align_width - width;
        let start = if rtl { free } else { 0.0 };
        let justify = !plan.mandatory
            && !last
            && plan.ellipsis.is_none()
            && plan.separators > 0
            && free > 0.0;
        let (left, extra) = match self.params.align {
            None => (start, 0.0),
            Some(Align::Left) => (0.0, 0.0),
            Some(Align::Right) => (free, 0.0),
            Some(Align::Center) => (free / 2.0, 0.0),
            Some(Align::End) => (if rtl { 0.0 } else { free }, 0.0),
            Some(Align::Justified) if justify => (0.0, free / plan.separators as f32),
            Some(Align::Justified) => (start, 0.0),
        };
        Placement {
            left,
            origin: if rtl { left - plan.hang_width } else { left },
            extra,
            width: width + extra * plan.separators as f32,
        }
    }

    fn emit_piece(
        &self,
        piece: &Piece,
        piece_index: usize,
        size: f32,
        extra: f32,
        mut pen: f32,
        glyphs: &mut Vec<Glyph>,
    ) -> f32 {
        let shaped = self.shaped;
        let face = &shaped.faces[usize::from(piece.face)];
        let columns = piece.columns(shaped, self.fragments);
        let tabs = shaped.tabs;
        let rtl = piece.level.is_rtl();
        let letter_spacing = self.params.letter_spacing;
        let range = piece.glyph_range();
        glyphs.reserve(range.len());
        let template = Glyph {
            face: face.face,
            location: face.location,
            id: 0,
            synthesis: face.synthesis,
            color: face.color,
            level: piece.level,
            x: 0.0,
            y: 0.0,
            advance: 0.0,
            font_size: size,
            start: 0,
            end: 0,
        };
        let mut place = |index: usize| {
            let flags = columns.flags[index];
            let separator = f32::from(u8::from(flags & flag::WORD_SEPARATOR != 0));
            let mut advance = columns.advances[index] * size + extra * separator;
            if tabs && flags & flag::TAB != 0 {
                advance = self.tab_advance(piece_index, index).unwrap_or(advance);
            }
            let spacing = if columns.ends_spaced_cluster(index) {
                letter_spacing
            } else {
                0.0
            };
            advance += spacing;
            let lead = if rtl { spacing } else { 0.0 };
            glyphs.push(Glyph {
                id: columns.ids[index],
                x: pen + lead + columns.x_offsets[index] * size,
                y: -columns.y_offsets[index] * size,
                advance,
                start: columns.clusters[index],
                end: columns.cluster_ends[index],
                ..template
            });
            pen += advance;
        };
        if rtl {
            range.rev().for_each(&mut place)
        } else {
            range.for_each(&mut place)
        }
        pen
    }

    fn tab_advance(&self, piece: usize, glyph: usize) -> Option<f32> {
        self.tabs
            .iter()
            .find(|tab| tab.piece as usize == piece && tab.glyph as usize == glyph)
            .map(|tab| tab.advance)
    }

    fn decorate(
        &self,
        piece: &Piece,
        size: f32,
        span: std::ops::Range<f32>,
        row_start: usize,
        decorations: &mut Vec<Decoration>,
    ) {
        let shaped = self.shaped;
        let style = &shaped.styles[usize::from(piece.style)];
        if !(style.underline || style.strikethrough) || span.end <= span.start {
            return;
        }
        let metrics: &EmMetrics = &shaped.faces[usize::from(piece.face)].metrics;
        let columns = piece.columns(shaped, self.fragments);
        let start = columns.clusters[piece.glyphs.start as usize];
        let kinds = [
            (
                style.underline,
                DecorationKind::Underline,
                metrics.underline_top,
                metrics.underline_thickness,
            ),
            (
                style.strikethrough,
                DecorationKind::Strikethrough,
                metrics.strikeout_top,
                metrics.strikeout_thickness,
            ),
        ];
        for (enabled, kind, top, thickness) in kinds {
            if !enabled {
                continue;
            }
            let decoration = Decoration {
                kind,
                x: span.start,
                width: span.end - span.start,
                y: top * size,
                thickness: thickness * size,
                start,
            };
            let joined = decorations[row_start..]
                .iter_mut()
                .rev()
                .find(|open| open.kind == kind);
            match joined {
                Some(open)
                    if (open.x + open.width - decoration.x).abs() < DECORATION_JOIN
                        && open.y == decoration.y
                        && open.thickness == decoration.thickness =>
                {
                    open.width += decoration.width;
                }
                _ => decorations.push(decoration),
            }
        }
    }
}
