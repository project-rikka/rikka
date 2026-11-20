use std::sync::Arc;

use rikka_text_layout::{Align, Family, FontStyle, Weight, Wrap};

use crate::layouts::layout_spaced;
use crate::shaping::TextShape;
use crate::span::{TextSpan, no_spans};
use crate::text::Text;
use crate::text_layout::TextLayout;

#[derive(Clone, Debug)]
#[must_use]
pub struct TextSpec {
    pub text: Text,
    pub font_size: f32,
    pub line_height: f32,
    pub family: Family,
    pub weight: Weight,
    pub style: FontStyle,
    pub wrap: Wrap,
    pub align: Option<Align>,
    pub tab_width: u16,
    pub ellipsize: bool,
    pub letter_spacing: f32,
    pub spans: Arc<[TextSpan]>,
}

impl TextSpec {
    pub fn new(text: impl Into<Text>, font_size: f32) -> Self {
        Self {
            text: text.into(),
            font_size,
            line_height: (font_size * 1.4).round(),
            family: Family::SansSerif,
            weight: Weight::NORMAL,
            style: FontStyle::Normal,
            wrap: Wrap::WordOrGlyph,
            align: None,
            tab_width: 8,
            ellipsize: false,
            letter_spacing: 0.0,
            spans: no_spans(),
        }
    }

    pub fn line_height(mut self, line_height: f32) -> Self {
        self.line_height = line_height;
        self
    }

    pub fn family(mut self, family: impl Into<Family>) -> Self {
        self.family = family.into();
        self
    }

    pub fn weight(mut self, weight: Weight) -> Self {
        self.weight = weight;
        self
    }

    pub fn style(mut self, style: FontStyle) -> Self {
        self.style = style;
        self
    }

    pub fn wrap(mut self, wrap: Wrap) -> Self {
        self.wrap = wrap;
        self
    }

    pub fn align(mut self, align: Align) -> Self {
        self.align = Some(align);
        self
    }

    pub fn tab_width(mut self, tab_width: u16) -> Self {
        self.tab_width = tab_width;
        self
    }

    pub fn ellipsize(mut self, ellipsize: bool) -> Self {
        self.ellipsize = ellipsize;
        self
    }

    pub fn letter_spacing(mut self, pixels: f32) -> Self {
        self.letter_spacing = pixels;
        self
    }

    pub fn spans(mut self, spans: impl Into<Arc<[TextSpan]>>) -> Self {
        self.spans = spans.into();
        self
    }

    pub fn as_shape(&self) -> TextShape<'_> {
        TextShape {
            text: &self.text,
            spans: &self.spans,
            font_size: self.font_size,
            line_height: self.line_height,
            family: &self.family,
            weight: self.weight,
            style: self.style,
            wrap: self.wrap,
            align: self.align,
            tab_width: self.tab_width,
            ellipsize: self.ellipsize,
        }
    }

    #[must_use]
    pub fn layout(&self, width: Option<f32>) -> Arc<TextLayout> {
        layout_spaced(self.as_shape(), self.letter_spacing, width)
    }
}
