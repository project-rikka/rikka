use rikka_types::{CornerRadii, Point, RoundedRect};

use crate::paint::ink::Ink;

#[derive(Clone, Copy, Debug, PartialEq)]
#[must_use]
pub struct Shadow {
    pub(crate) blur: f32,
    pub(crate) spread: f32,
    pub(crate) offset: Point,
    pub(crate) ink: Ink,
}

impl Shadow {
    pub fn new(blur: f32, ink: impl Into<Ink>) -> Self {
        Self {
            blur: blur.max(0.0),
            spread: 0.0,
            offset: Point::ZERO,
            ink: ink.into(),
        }
    }

    pub fn spread(mut self, spread: f32) -> Self {
        self.spread = spread;
        self
    }

    pub fn offset(mut self, x: f32, y: f32) -> Self {
        self.offset = Point::new(x, y);
        self
    }

    pub(crate) fn cast(&self, shape: RoundedRect) -> RoundedRect {
        let rect = shape.rect.inflate(self.spread).translate(self.offset);
        let grown = |radius: f32| match radius > 0.0 {
            true => (radius + self.spread).max(0.0),
            false => 0.0,
        };
        let radii = shape.radii;
        RoundedRect::new(
            rect,
            CornerRadii::new(
                grown(radii.top_left),
                grown(radii.top_right),
                grown(radii.bottom_right),
                grown(radii.bottom_left),
            ),
        )
    }

    pub(crate) fn sigma(&self) -> f32 {
        self.blur * 0.5
    }
}
