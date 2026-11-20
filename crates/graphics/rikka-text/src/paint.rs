use std::sync::Arc;

use rikka_svg::ColorScheme;
use rikka_types::Color;

use crate::span::{Inks, TextSpan};
use crate::text_layout::TextLayout;

#[derive(Clone, Debug)]
pub struct TextPaint {
    pub layout: Arc<TextLayout>,
    pub inks: Inks,
    pub scale: f32,
    pub color_scheme: ColorScheme,
}

impl TextPaint {
    #[must_use]
    pub fn new(layout: Arc<TextLayout>) -> Self {
        Self {
            layout,
            inks: Inks::default(),
            scale: 1.0,
            color_scheme: ColorScheme::default(),
        }
    }

    #[must_use]
    pub fn with_spans(mut self, spans: &[TextSpan]) -> Self {
        self.inks = Inks::from_spans(self.layout.text(), spans);
        self
    }

    #[must_use]
    pub fn scaled(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    #[must_use]
    pub fn with_color_scheme(mut self, scheme: ColorScheme) -> Self {
        self.color_scheme = scheme;
        self
    }

    #[must_use]
    pub fn ink(&self, offset: Option<usize>) -> Option<Color> {
        self.inks.at(offset?)
    }
}

impl PartialEq for TextPaint {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.layout, &other.layout)
            && self.inks == other.inks
            && self.scale == other.scale
            && self.color_scheme == other.color_scheme
    }
}

impl From<Arc<TextLayout>> for TextPaint {
    fn from(layout: Arc<TextLayout>) -> Self {
        Self::new(layout)
    }
}
