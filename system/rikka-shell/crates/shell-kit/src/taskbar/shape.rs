use rikka_settings::session::appearance::TaskbarPosition;
use rikka_theme::Corner;
use rikka_types::{CornerRadii, Point, Rect, RoundedRect};
use rikka_ui::Inset;

use crate::taskbar::Edge;
use crate::{ExclusiveZone, PointerMode};

pub const DEPTH: f32 = 40.0;

pub const BORDER: f32 = 10.0;

pub const CORNER: f32 = Corner::Window.radius();

const HANG: f32 = 300.0;

const SIDE_HANG: f32 = 360.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    edge: Edge,
}

impl Shape {
    #[must_use]
    pub const fn new(position: TaskbarPosition) -> Shape {
        Shape {
            edge: Edge::new(position),
        }
    }

    #[must_use]
    pub fn edge(self) -> Edge {
        self.edge
    }

    #[must_use]
    pub fn corner(self) -> f32 {
        CORNER
    }

    #[must_use]
    pub fn insets(self) -> Inset {
        self.edge.inset(DEPTH, BORDER, BORDER, BORDER)
    }

    #[must_use]
    pub fn strip(self) -> Inset {
        self.edge.inset(DEPTH, 0.0, 0.0, 0.0)
    }

    #[must_use]
    pub fn desktop(self, bounds: Rect) -> RoundedRect {
        let insets = self.insets();
        let left = insets.left().min(bounds.width());
        let top = insets.top().min(bounds.height());
        let rect = Rect::new(
            bounds.min_x() + left,
            bounds.min_y() + top,
            (bounds.width() - left - insets.right()).max(0.0),
            (bounds.height() - top - insets.bottom()).max(0.0),
        );
        RoundedRect::new(rect, CornerRadii::uniform(CORNER))
    }

    #[must_use]
    pub fn covers(self, bounds: Rect, at: Point) -> bool {
        !self.desktop(bounds).rect.contains(at)
    }

    #[must_use]
    pub fn extent(self) -> f32 {
        match self.edge.is_vertical() {
            true => DEPTH + SIDE_HANG,
            false => DEPTH + HANG,
        }
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
