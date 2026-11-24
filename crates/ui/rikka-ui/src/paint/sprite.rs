use rikka_renderer::Transform;
use rikka_types::{Color, CornerRadii, Rect};

use crate::paint::ink::Ink;
use crate::picture::ImageSource;

#[derive(Clone, Debug, PartialEq)]
pub struct Sprite {
    pub(crate) rect: Rect,
    pub(crate) source: ImageSource,
    pub(crate) placeholder: Option<ImageSource>,
    pub(crate) radii: CornerRadii,
    pub(crate) turn: f32,
    pub(crate) mirror: Transform,
    pub(crate) pixelated: bool,
    pub(crate) tint: Ink,
}

impl Sprite {
    pub fn new(rect: Rect, source: impl Into<ImageSource>) -> Self {
        Self {
            rect,
            source: source.into(),
            placeholder: None,
            radii: CornerRadii::ZERO,
            turn: 0.0,
            mirror: Transform::Normal,
            pixelated: false,
            tint: Ink::Solid(Color::WHITE),
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<ImageSource>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    pub fn radius(mut self, radii: impl Into<CornerRadii>) -> Self {
        self.radii = radii.into();
        self
    }

    pub fn turned(mut self, angle: f32) -> Self {
        self.turn = angle;
        self
    }

    pub fn mirrored(mut self, horizontally: bool, vertically: bool) -> Self {
        self.mirror = match (horizontally, vertically) {
            (false, false) => Transform::Normal,
            (true, false) => Transform::Flipped,
            (false, true) => Transform::Flipped180,
            (true, true) => Transform::Rotate180,
        };
        self
    }

    pub fn tinted(mut self, tint: impl Into<Ink>) -> Self {
        self.tint = tint.into();
        self
    }

    pub fn pixelated(mut self) -> Self {
        self.pixelated = true;
        self
    }
}
