use std::fmt;
use std::sync::{Arc, LazyLock};

use rikka_text_unicode::Level;

use crate::fonts::{EmMetrics, FaceInstance};
use crate::shaping::columns::GlyphColumns;
use crate::shaping::run::{Run, RunStyle};
use crate::shaping::segments::Segment;
use crate::style::Caps;

#[derive(Clone)]
pub struct ShapedParagraph(pub(crate) Arc<Shaped>);

#[derive(Debug, Default)]
pub(crate) struct Shaped {
    pub(crate) text: Box<str>,
    pub(crate) base_level: Level,
    pub(crate) tab_width: u16,
    pub(crate) caps: Caps,
    pub(crate) faces: Box<[Arc<FaceInstance>]>,
    pub(crate) styles: Box<[RunStyle]>,
    pub(crate) runs: Box<[Run]>,
    pub(crate) glyphs: GlyphColumns,
    pub(crate) segments: Box<[Segment]>,
    pub(crate) bidi: bool,
    pub(crate) tabs: bool,
}

impl ShapedParagraph {
    pub(crate) fn empty() -> Self {
        static EMPTY: LazyLock<ShapedParagraph> =
            LazyLock::new(|| ShapedParagraph(Arc::new(Shaped::default())));
        EMPTY.clone()
    }

    pub fn text(&self) -> &str {
        &self.0.text
    }

    pub fn is_rtl(&self) -> bool {
        self.0.base_level.is_rtl()
    }

    pub fn base_level(&self) -> Level {
        self.0.base_level
    }

    pub fn is_bidi(&self) -> bool {
        self.0.bidi
    }

    pub fn glyph_count(&self) -> usize {
        self.0.glyphs.len()
    }

    pub fn heap_bytes(&self) -> usize {
        let shaped = &*self.0;
        size_of::<Shaped>()
            + shaped.text.len()
            + shaped.faces.len() * size_of::<Arc<FaceInstance>>()
            + shaped.styles.len() * size_of::<RunStyle>()
            + shaped.runs.len() * size_of::<Run>()
            + shaped.glyphs.heap_bytes()
            + shaped.segments.len() * size_of::<Segment>()
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    #[inline]
    pub(crate) fn shaped(&self) -> &Shaped {
        &self.0
    }
}

impl Shaped {
    #[inline]
    pub(crate) fn run_of_glyph(&self, glyph: usize) -> usize {
        self.runs
            .partition_point(|run| run.glyphs.end as usize <= glyph)
    }

    #[inline]
    pub(crate) fn run_at_offset(&self, offset: u32) -> usize {
        self.runs.partition_point(|run| run.text.end <= offset)
    }

    pub(crate) fn base_face(&self) -> Option<u16> {
        self.styles.first()?.primary
    }

    pub(crate) fn base_metrics(&self) -> &EmMetrics {
        self.base_face()
            .map_or(&EmMetrics::FALLBACK, |face| self.metrics_of(face))
    }

    pub(crate) fn strut_metrics(&self, style: u16, face: u16) -> &EmMetrics {
        self.metrics_of(self.styles[usize::from(style)].primary.unwrap_or(face))
    }

    fn metrics_of(&self, face: u16) -> &EmMetrics {
        &self.faces[usize::from(face)].metrics
    }
}

impl fmt::Debug for ShapedParagraph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShapedParagraph")
            .field("text", &self.0.text)
            .field("runs", &self.0.runs.len())
            .field("glyphs", &self.0.glyphs.len())
            .finish_non_exhaustive()
    }
}
