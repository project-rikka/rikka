use std::hash::{Hash, Hasher};
use std::sync::Arc;

use rikka_text_layout::{Align, Family, FontStyle, LayoutParams, Weight, Wrap};
use rustc_hash::FxHasher;

use crate::span::TextSpan;
use crate::text::Text;

#[derive(Clone, Copy, Debug)]
pub struct TextShape<'a> {
    pub text: &'a Text,
    pub spans: &'a Arc<[TextSpan]>,
    pub font_size: f32,
    pub line_height: f32,
    pub family: &'a Family,
    pub weight: Weight,
    pub style: FontStyle,
    pub wrap: Wrap,
    pub align: Option<Align>,
    pub tab_width: u16,
    pub ellipsize: bool,
}

impl TextShape<'_> {
    pub(crate) fn layout_request(&self, width: Option<f32>, letter_spacing: f32) -> LayoutRequest {
        LayoutRequest {
            font_size: self.font_size.max(1.0),
            line_height: self.line_height.max(1.0),
            width,
            wrap: self.wrap,
            align: self.align,
            ellipsize: self.ellipsize && self.wrap == Wrap::None,
            letter_spacing: match letter_spacing.is_finite() {
                true => letter_spacing,
                false => 0.0,
            },
        }
    }

    pub(crate) fn shape_digest(&self) -> u64 {
        let mut hasher = FxHasher::default();
        hasher.write_u64(self.text.digest());
        self.family.hash(&mut hasher);
        self.weight.hash(&mut hasher);
        self.style.hash(&mut hasher);
        self.tab_width.hash(&mut hasher);
        for span in self.spans.iter().filter(|span| span.shapes()) {
            span.hash_shape(&mut hasher);
        }
        hasher.finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LayoutRequest {
    pub(crate) font_size: f32,
    pub(crate) line_height: f32,
    pub(crate) width: Option<f32>,
    pub(crate) wrap: Wrap,
    pub(crate) align: Option<Align>,
    pub(crate) ellipsize: bool,
    pub(crate) letter_spacing: f32,
}

impl LayoutRequest {
    pub(crate) fn digest(&self, shape: u64) -> u64 {
        let mut hasher = FxHasher::default();
        hasher.write_u64(shape);
        self.font_size.to_bits().hash(&mut hasher);
        self.line_height.to_bits().hash(&mut hasher);
        self.wrap.hash(&mut hasher);
        self.align.hash(&mut hasher);
        self.ellipsize.hash(&mut hasher);
        self.letter_spacing.to_bits().hash(&mut hasher);
        hasher.finish()
    }

    pub(crate) fn is_compatible(&self, other: &LayoutRequest) -> bool {
        self.font_size == other.font_size
            && self.line_height == other.line_height
            && self.wrap == other.wrap
            && self.align == other.align
            && self.ellipsize == other.ellipsize
            && self.letter_spacing == other.letter_spacing
    }

    pub(crate) fn params(&self, ellipsize: bool) -> LayoutParams {
        LayoutParams {
            font_size: self.font_size,
            line_height: self.line_height,
            width: self.width,
            wrap: self.wrap,
            align: self.align,
            ellipsize,
            letter_spacing: self.letter_spacing,
        }
    }
}
