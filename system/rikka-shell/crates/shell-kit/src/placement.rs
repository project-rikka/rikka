use rikka_types::{Point, Rect, Size};

#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    pub name: String,
    pub position: Point,
    pub size: Size,
    pub scale: f32,
}

impl Default for Placement {
    fn default() -> Placement {
        Placement {
            name: String::new(),
            position: Point::ZERO,
            size: Size::ZERO,
            scale: 1.0,
        }
    }
}

impl Placement {
    #[must_use]
    pub fn to_desktop(&self, local: Point) -> Point {
        local + self.position
    }

    #[must_use]
    pub fn to_surface(&self, desktop: Rect) -> Rect {
        desktop.translate(-self.position)
    }
}
