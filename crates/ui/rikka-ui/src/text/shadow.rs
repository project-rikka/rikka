use rikka_types::{Color, Point};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextShadow {
    pub offset: Point,
    pub color: Color,
}

impl TextShadow {
    pub fn new(x: f32, y: f32, color: Color) -> Self {
        Self {
            offset: Point::new(x, y),
            color,
        }
    }
}
