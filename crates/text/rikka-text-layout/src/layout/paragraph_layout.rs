use std::borrow::Cow;

use smallvec::SmallVec;

use crate::layout::breaking::{widest_cluster, widest_segment};
use crate::layout::decoration::Decoration;
use crate::layout::engine::layout_into;
use crate::layout::glyph::Glyph;
use crate::layout::params::{LayoutParams, Wrap};
use crate::layout::row::Row;
use crate::shaping::ShapedParagraph;

#[derive(Clone, Debug)]
pub struct ParagraphLayout {
    pub(crate) paragraph: ShapedParagraph,
    pub(crate) params: LayoutParams,
    pub(crate) rows: SmallVec<[Row; 1]>,
    pub(crate) glyphs: Vec<Glyph>,
    pub(crate) decorations: Vec<Decoration>,
    pub(crate) width: f32,
}

impl ParagraphLayout {
    pub fn new() -> Self {
        Self {
            paragraph: ShapedParagraph::empty(),
            params: LayoutParams::new(0.0, 0.0),
            rows: SmallVec::new(),
            glyphs: Vec::new(),
            decorations: Vec::new(),
            width: 0.0,
        }
    }

    pub(crate) fn reset(&mut self, paragraph: &ShapedParagraph, params: LayoutParams) {
        if !self.paragraph.ptr_eq(paragraph) {
            self.paragraph = paragraph.clone();
        }
        self.params = params;
        self.rows.clear();
        self.glyphs.clear();
        self.decorations.clear();
        self.width = 0.0;
    }

    pub fn paragraph(&self) -> &ShapedParagraph {
        &self.paragraph
    }

    pub fn text(&self) -> &str {
        self.paragraph.text()
    }

    pub fn params(&self) -> &LayoutParams {
        &self.params
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn glyphs(&self, row: &Row) -> &[Glyph] {
        &self.glyphs[row.glyph_range()]
    }

    pub fn decorations(&self, row: &Row) -> &[Decoration] {
        &self.decorations[row.decoration_range()]
    }

    pub fn all_glyphs(&self) -> &[Glyph] {
        &self.glyphs
    }

    pub fn rows_between(&self, top: f32, bottom: f32) -> &[Row] {
        let first = self.rows.partition_point(|row| row.bottom() <= top);
        let last = self.rows.partition_point(|row| row.top < bottom);
        &self.rows[first..last.max(first)]
    }

    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn height(&self) -> f32 {
        self.rows.len() as f32 * self.params.line_height
    }

    pub fn is_elided(&self) -> bool {
        self.rows.iter().any(Row::is_elided)
    }

    pub fn visible_text(&self) -> Cow<'_, str> {
        let text = self.text();
        if !self.is_elided() {
            return Cow::Borrowed(text);
        }
        let mut visible_text = String::with_capacity(text.len());
        for row in &self.rows {
            visible_text.push_str(&text[row.text_range()]);
            visible_text.push_str(row.ellipsis().unwrap_or_default());
        }
        Cow::Owned(visible_text)
    }

    pub fn heap_bytes(&self) -> usize {
        let rows = match self.rows.spilled() {
            true => self.rows.capacity() * size_of::<Row>(),
            false => 0,
        };
        rows + self.glyphs.capacity() * size_of::<Glyph>()
            + self.decorations.capacity() * size_of::<Decoration>()
    }
}

impl Default for ParagraphLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl ShapedParagraph {
    pub fn layout(&self, params: &LayoutParams) -> ParagraphLayout {
        let mut layout = ParagraphLayout::new();
        self.layout_into(params, &mut layout);
        layout
    }

    pub fn layout_into(&self, params: &LayoutParams, layout: &mut ParagraphLayout) {
        layout_into(self, params, layout);
    }

    pub fn min_content_width(&self, params: &LayoutParams) -> f32 {
        let params = params.sanitized();
        match params.wrap {
            Wrap::None => self
                .layout(&LayoutParams {
                    width: None,
                    ..params
                })
                .width(),
            Wrap::Word | Wrap::WordOrGlyph => widest_segment(self.shaped(), &params),
            Wrap::Glyph => widest_cluster(self.shaped(), &params),
        }
    }
}
