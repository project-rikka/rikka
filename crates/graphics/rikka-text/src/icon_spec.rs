use rikka_text_layout::Weight;

use crate::icon_font::IconFont;

#[derive(Clone, Copy, Debug, PartialEq)]
#[must_use]
pub struct IconSpec {
    pub glyph: char,
    pub font: IconFont,
    pub size: f32,
    pub optical_size: f32,
    pub fill: f32,
    pub weight: Weight,
    pub grade: f32,
}

impl IconSpec {
    pub fn new(glyph: char, size: f32) -> Self {
        Self {
            glyph,
            font: IconFont::MaterialSymbols,
            size,
            optical_size: size,
            fill: 0.0,
            weight: Weight::NORMAL,
            grade: 0.0,
        }
    }

    pub fn scaled(self, scale: f32) -> Self {
        Self {
            size: self.size * scale,
            ..self
        }
    }

    pub fn font(mut self, font: IconFont) -> Self {
        self.font = font;
        self
    }

    pub fn fill(mut self, fill: f32) -> Self {
        self.fill = fill.clamp(0.0, 1.0);
        self
    }

    pub fn weight(mut self, weight: Weight) -> Self {
        self.weight = weight;
        self
    }

    pub fn grade(mut self, grade: f32) -> Self {
        self.grade = grade;
        self
    }
}
