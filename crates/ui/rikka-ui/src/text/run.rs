use std::sync::{Arc, LazyLock};

use rikka_text::{Align, Family, FontStyle, Weight, Wrap};
use rikka_text::{Text, TextShape, TextSpan};

#[derive(Clone, Debug, PartialEq)]
pub struct TextRun {
    pub content: Text,
    pub size: f32,
    pub line_height: f32,
    pub family: Family,
    pub weight: Weight,
    pub style: FontStyle,
    pub wrap: Wrap,
    pub align: Option<Align>,
    pub tab_width: u16,
    pub ellipsize: bool,
    pub spans: Arc<[TextSpan]>,
}

impl TextRun {
    pub fn content(mut self, content: impl Into<Arc<str>>) -> Self {
        self.content = Text::new(content);
        self
    }

    pub fn weight(mut self, weight: Weight) -> Self {
        self.weight = weight;
        self
    }

    pub fn medium(self) -> Self {
        self.weight(crate::font::MEDIUM)
    }

    pub fn no_wrap(mut self) -> Self {
        self.wrap = Wrap::None;
        self
    }

    pub fn ellipsized(mut self) -> Self {
        self.ellipsize = true;
        self
    }

    pub fn italic(mut self) -> Self {
        self.style = FontStyle::Italic;
        self
    }

    pub fn line_height(mut self, line_height: f32) -> Self {
        self.line_height = line_height;
        self
    }

    pub fn family(mut self, family: Family) -> Self {
        self.family = family;
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

    pub fn spans(mut self, spans: impl Into<Arc<[TextSpan]>>) -> Self {
        self.spans = spans.into();
        self
    }

    pub fn new(content: impl Into<Arc<str>>, size: f32) -> Self {
        Self {
            content: Text::new(content),
            size,
            line_height: (size * 1.4).round(),
            family: Family::SansSerif,
            weight: Weight::NORMAL,
            style: FontStyle::Normal,
            wrap: Wrap::WordOrGlyph,
            align: None,
            tab_width: 8,
            ellipsize: false,
            spans: no_spans(),
        }
    }

    pub fn as_shape(&self) -> TextShape<'_> {
        TextShape {
            text: &self.content,
            spans: &self.spans,
            font_size: self.size,
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
}

fn no_spans() -> Arc<[TextSpan]> {
    static EMPTY: LazyLock<Arc<[TextSpan]>> = LazyLock::new(|| Arc::from(Vec::new()));
    EMPTY.clone()
}
