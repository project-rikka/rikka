use crate::shaping::shaped_paragraph::ShapedParagraph;
use crate::shaping::shaper::shape;
use crate::style::{StyleSpan, TextStyle};

const DEFAULT_TAB_WIDTH: u16 = 8;

#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct Paragraph<'a> {
    pub(crate) text: &'a str,
    pub(crate) style: &'a TextStyle,
    pub(crate) spans: &'a [StyleSpan],
    pub(crate) tab_width: u16,
}

impl<'a> Paragraph<'a> {
    pub fn new(text: &'a str, style: &'a TextStyle) -> Self {
        Self {
            text,
            style,
            spans: &[],
            tab_width: DEFAULT_TAB_WIDTH,
        }
    }

    pub fn spans(mut self, spans: &'a [StyleSpan]) -> Self {
        self.spans = spans;
        self
    }

    pub fn tab_width(mut self, tab_width: u16) -> Self {
        self.tab_width = tab_width;
        self
    }

    pub fn shape(&self) -> ShapedParagraph {
        shape(self)
    }
}
