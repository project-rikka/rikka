use std::ops::Range;

use rikka_text_catalog::{Family, FontStyle, Weight};

#[derive(Clone, Debug, Default, PartialEq)]
#[must_use]
pub struct StyleSpan {
    pub(crate) range: Range<usize>,
    pub(crate) family: Option<Family>,
    pub(crate) weight: Option<Weight>,
    pub(crate) style: Option<FontStyle>,
    pub(crate) font_size: Option<f32>,
    pub(crate) underline: bool,
    pub(crate) strikethrough: bool,
}

impl StyleSpan {
    pub fn new(range: Range<usize>) -> Self {
        Self {
            range,
            ..Self::default()
        }
    }

    pub fn family(mut self, family: impl Into<Family>) -> Self {
        self.family = Some(family.into());
        self
    }

    pub fn weight(mut self, weight: Weight) -> Self {
        self.weight = Some(weight);
        self
    }

    pub fn style(mut self, style: FontStyle) -> Self {
        self.style = Some(style);
        self
    }

    pub fn font_size(mut self, font_size: f32) -> Self {
        self.font_size = Some(font_size);
        self
    }

    pub fn underline(mut self, underline: bool) -> Self {
        self.underline = underline;
        self
    }

    pub fn strikethrough(mut self, strikethrough: bool) -> Self {
        self.strikethrough = strikethrough;
        self
    }

    pub(crate) fn changes_font(&self) -> bool {
        self.family.is_some() || self.weight.is_some() || self.style.is_some()
    }
}
