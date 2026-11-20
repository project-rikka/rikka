use std::cell::RefCell;

use rikka_text_unicode::Level;

use crate::layout::breaking::{Breaker, WIDTH_EPSILON};
use crate::layout::ellipsis::{MAX_ATTEMPTS, find_cut};
use crate::layout::emit::{Emitter, RowOutput};
use crate::layout::paragraph_layout::ParagraphLayout;
use crate::layout::params::LayoutParams;
use crate::layout::pieces::{Pieces, TabAdvance};
use crate::layout::plan::{Piece, RowPlan, Source};
use crate::shaping::{GlyphColumns, Shaped, ShapedParagraph};

#[derive(Default)]
struct Scratch {
    plans: Vec<RowPlan>,
    pieces: Vec<Piece>,
    fragments: GlyphColumns,
    tabs: Vec<TabAdvance>,
    levels: Vec<Level>,
    order: Vec<usize>,
}

#[derive(Clone, Copy)]
struct Checkpoint {
    pieces: usize,
    fragments: usize,
    tabs: usize,
}

thread_local! {
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::default());
}

pub(crate) fn layout_into(
    paragraph: &ShapedParagraph,
    params: &LayoutParams,
    out: &mut ParagraphLayout,
) {
    let params = params.sanitized();
    out.reset(paragraph, params);
    SCRATCH.with(|scratch| {
        scratch
            .borrow_mut()
            .layout(paragraph.shaped(), &params, out)
    });
}

impl Scratch {
    fn layout(&mut self, shaped: &Shaped, params: &LayoutParams, out: &mut ParagraphLayout) {
        Breaker::new(shaped, params, &mut self.plans).run();
        self.pieces.clear();
        self.fragments.clear();
        self.tabs.clear();
        let mut widest: f32 = 0.0;
        for index in 0..self.plans.len() {
            self.prepare(shaped, params, index);
            widest = widest.max(self.plans[index].content_width);
        }
        let align_width = params
            .width
            .filter(|width| width.is_finite())
            .unwrap_or(widest);
        let mut emitter = Emitter {
            shaped,
            params,
            pieces: &self.pieces,
            fragments: &self.fragments,
            tabs: &self.tabs,
            levels: &mut self.levels,
            order: &mut self.order,
            align_width,
        };
        let mut output = RowOutput {
            rows: &mut out.rows,
            glyphs: &mut out.glyphs,
            decorations: &mut out.decorations,
        };
        let count = self.plans.len();
        for (index, plan) in self.plans.iter().enumerate() {
            emitter.emit_row(index, plan, index + 1 == count, &mut output);
        }
        out.width = widest;
    }

    fn prepare(&mut self, shaped: &Shaped, params: &LayoutParams, index: usize) {
        let checkpoint = self.checkpoint();
        let plan = &self.plans[index];
        let (start, end, content_end) = (plan.start, plan.end, plan.content_end);
        let mut pieces = self.pieces_of(shaped, params);
        let range = pieces.collect(start..end, content_end);
        let measured = pieces.measure(range.clone());
        let plan = &mut self.plans[index];
        plan.pieces = range;
        plan.content_width = measured.content;
        plan.hang_width = measured.hanging;
        let width = params.width.unwrap_or(f32::INFINITY);
        if params.elides() && measured.content > width + WIDTH_EPSILON {
            self.elide(shaped, params, index, checkpoint, width);
        }
    }

    fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            pieces: self.pieces.len(),
            fragments: self.fragments.len(),
            tabs: self.tabs.len(),
        }
    }

    fn rewind(&mut self, checkpoint: Checkpoint) {
        self.pieces.truncate(checkpoint.pieces);
        self.fragments.truncate(checkpoint.fragments);
        self.tabs.truncate(checkpoint.tabs);
    }

    fn pieces_of<'a>(&'a mut self, shaped: &'a Shaped, params: &LayoutParams) -> Pieces<'a> {
        Pieces {
            shaped,
            font_size: params.font_size,
            letter_spacing: params.letter_spacing,
            fragments: &mut self.fragments,
            pieces: &mut self.pieces,
            tabs: &mut self.tabs,
        }
    }

    fn elide(
        &mut self,
        shaped: &Shaped,
        params: &LayoutParams,
        index: usize,
        checkpoint: Checkpoint,
        width: f32,
    ) {
        let font_size = params.font_size;
        let start = self.plans[index].start;
        let mut ceiling = self.plans[index].end;
        let mut attempt = 0;
        loop {
            attempt += 1;
            let cut = find_cut(shaped, start..ceiling, params, width);
            self.rewind(checkpoint);
            let mut pieces = self.pieces_of(shaped, params);
            let range = pieces.collect(start..cut.end, cut.end);
            let measured = pieces.measure(range.clone());
            let ellipsis_width = cut.ellipsis_width(shaped, font_size);
            let fits = measured.content + ellipsis_width <= width + WIDTH_EPSILON;
            if !fits && cut.end > start && attempt < MAX_ATTEMPTS {
                ceiling = cut.last_start;
                continue;
            }
            let ellipsis = cut.ellipsis(shaped);
            let mut range = range;
            if let Some((face, ellipsis)) = ellipsis {
                let first = self.fragments.len();
                for glyph in ellipsis.glyphs.iter() {
                    self.fragments.push_ellipsis_glyph(glyph, cut.end);
                }
                self.pieces.push(Piece {
                    source: Source::Fragment,
                    glyphs: first as u32..self.fragments.len() as u32,
                    face,
                    style: cut.style,
                    level: shaped.base_level,
                    script: rikka_text_unicode::Script::Common,
                    hanging: false,
                    width: ellipsis_width,
                });
                range.end = self.pieces.len() as u32;
            }
            let plan = &mut self.plans[index];
            plan.end = cut.end;
            plan.content_end = cut.end;
            plan.pieces = range;
            plan.content_width = measured.content + ellipsis_width;
            plan.hang_width = 0.0;
            plan.ellipsis = Some(ellipsis.map_or("", |(_, ellipsis)| ellipsis.text));
            return;
        }
    }
}
