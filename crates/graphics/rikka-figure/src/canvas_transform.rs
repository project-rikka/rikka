use rikka_types::Point;

use crate::affine::Affine;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasTransform {
    pub scale: f32,
    pub offset: Point,
}

impl Default for CanvasTransform {
    fn default() -> CanvasTransform {
        CanvasTransform::IDENTITY
    }
}

impl CanvasTransform {
    pub const IDENTITY: CanvasTransform = CanvasTransform {
        scale: 1.0,
        offset: Point::ZERO,
    };

    #[must_use]
    pub fn new(scale: f32, offset: Point) -> CanvasTransform {
        CanvasTransform { scale, offset }
    }

    #[must_use]
    pub fn affine(&self) -> Affine {
        Affine::scale(f64::from(self.scale), f64::from(self.scale)).then(Affine::translation(
            f64::from(self.offset.x),
            f64::from(self.offset.y),
        ))
    }
}
