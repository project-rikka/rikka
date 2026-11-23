use rikka_theme::Corner;
use rikka_types::{CornerRadii, Point, Rect, RoundedRect};

use crate::{ExclusiveZone, PointerMode};

pub const DEPTH: f32 = 40.0;

pub const BORDER: f32 = 10.0;

pub const CORNER: f32 = Corner::Window.radius();

const HANG: f32 = 300.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape;

impl Shape {
    #[must_use]
    pub fn corner(self) -> f32 {
        CORNER
    }

    #[must_use]
    pub fn border(self) -> f32 {
        BORDER
    }

    #[must_use]
    pub fn desktop(self, bounds: Rect) -> RoundedRect {
        let top = DEPTH.min(bounds.height());
        let rect = Rect::new(
            bounds.min_x() + BORDER,
            bounds.min_y() + top,
            (bounds.width() - BORDER * 2.0).max(0.0),
            (bounds.height() - top - BORDER).max(0.0),
        );
        RoundedRect::new(rect, CornerRadii::uniform(CORNER))
    }

    #[must_use]
    pub fn covers(self, bounds: Rect, at: Point) -> bool {
        !self.desktop(bounds).rect.contains(at)
    }

    #[must_use]
    pub fn height(self) -> f32 {
        DEPTH + HANG
    }

    #[must_use]
    pub fn reserved(self) -> f32 {
        DEPTH
    }

    #[must_use]
    pub fn exclusive(self) -> ExclusiveZone {
        ExclusiveZone::Fixed(DEPTH)
    }

    #[must_use]
    pub fn pointer(self) -> PointerMode {
        PointerMode::Panes
    }
}
