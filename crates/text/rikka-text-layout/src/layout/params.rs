#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Wrap {
    None,
    Word,
    Glyph,
    #[default]
    WordOrGlyph,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Align {
    Left,
    Right,
    Center,
    Justified,
    End,
}

const TIGHTEST_LETTER_SPACING_EM: f32 = -0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutParams {
    pub font_size: f32,
    pub line_height: f32,
    pub width: Option<f32>,
    pub wrap: Wrap,
    pub align: Option<Align>,
    pub ellipsize: bool,
    pub letter_spacing: f32,
}

impl LayoutParams {
    pub fn new(font_size: f32, line_height: f32) -> Self {
        Self {
            font_size,
            line_height,
            width: None,
            wrap: Wrap::WordOrGlyph,
            align: None,
            ellipsize: false,
            letter_spacing: 0.0,
        }
    }

    pub(crate) fn sanitized(&self) -> Self {
        let finite = |value: f32| {
            if value.is_finite() {
                value.max(0.0)
            } else {
                0.0
            }
        };
        let font_size = finite(self.font_size);
        let letter_spacing = if self.letter_spacing.is_finite() {
            self.letter_spacing
                .max(font_size * TIGHTEST_LETTER_SPACING_EM)
        } else {
            0.0
        };
        Self {
            font_size,
            line_height: finite(self.line_height),
            letter_spacing,
            width: self
                .width
                .filter(|width| !width.is_nan())
                .map(|width| width.max(0.0)),
            ..*self
        }
    }

    pub(crate) fn limit(&self) -> f32 {
        match (self.wrap, self.width) {
            (Wrap::None, _) | (_, None) => f32::INFINITY,
            (_, Some(width)) => width,
        }
    }

    pub(crate) fn elides(&self) -> bool {
        self.ellipsize && self.wrap == Wrap::None && self.width.is_some_and(f32::is_finite)
    }
}
