use rikka_settings::session::appearance::TaskbarPosition;
use rikka_types::{CornerRadii, Point, Rect};
use rikka_ui::{Axis, Inset, Justify};

use crate::Anchor;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Edge(TaskbarPosition);

impl Edge {
    pub const ALL: [Edge; 4] = [
        Edge(TaskbarPosition::Top),
        Edge(TaskbarPosition::Bottom),
        Edge(TaskbarPosition::Left),
        Edge(TaskbarPosition::Right),
    ];

    #[must_use]
    pub const fn new(position: TaskbarPosition) -> Edge {
        Edge(position)
    }

    #[must_use]
    pub fn position(self) -> TaskbarPosition {
        self.0
    }

    #[must_use]
    pub fn opposite(self) -> Edge {
        Edge(match self.0 {
            TaskbarPosition::Top => TaskbarPosition::Bottom,
            TaskbarPosition::Bottom => TaskbarPosition::Top,
            TaskbarPosition::Left => TaskbarPosition::Right,
            TaskbarPosition::Right => TaskbarPosition::Left,
        })
    }

    #[must_use]
    pub fn sides(self) -> [Edge; 2] {
        match self.is_vertical() {
            true => [Edge(TaskbarPosition::Top), Edge(TaskbarPosition::Bottom)],
            false => [Edge(TaskbarPosition::Left), Edge(TaskbarPosition::Right)],
        }
    }

    #[must_use]
    pub fn is_vertical(self) -> bool {
        matches!(self.0, TaskbarPosition::Left | TaskbarPosition::Right)
    }

    #[must_use]
    pub fn axis(self) -> Axis {
        match self.is_vertical() {
            true => Axis::Vertical,
            false => Axis::Horizontal,
        }
    }

    #[must_use]
    pub fn anchor(self) -> Anchor {
        match self.0 {
            TaskbarPosition::Top => Anchor::TOP,
            TaskbarPosition::Bottom => Anchor::BOTTOM,
            TaskbarPosition::Left => Anchor::LEFT,
            TaskbarPosition::Right => Anchor::RIGHT,
        }
    }

    #[must_use]
    pub fn outward(self, distance: f32) -> Point {
        match self.0 {
            TaskbarPosition::Top => Point::new(0.0, -distance),
            TaskbarPosition::Bottom => Point::new(0.0, distance),
            TaskbarPosition::Left => Point::new(-distance, 0.0),
            TaskbarPosition::Right => Point::new(distance, 0.0),
        }
    }

    #[must_use]
    pub fn justify(self) -> Justify {
        match self.0 {
            TaskbarPosition::Top | TaskbarPosition::Left => Justify::Start,
            TaskbarPosition::Bottom | TaskbarPosition::Right => Justify::End,
        }
    }

    #[must_use]
    pub fn length(self, bounds: Rect) -> f32 {
        self.axis().room(bounds)
    }

    #[must_use]
    pub fn place(self, bounds: Rect, local: Rect) -> Rect {
        let (along, across) = (local.x(), local.y());
        let (length, depth) = (local.width(), local.height());
        match self.0 {
            TaskbarPosition::Top => {
                Rect::new(bounds.x() + along, bounds.y() + across, length, depth)
            }
            TaskbarPosition::Bottom => Rect::new(
                bounds.x() + along,
                bounds.max_y() - across - depth,
                length,
                depth,
            ),
            TaskbarPosition::Left => {
                Rect::new(bounds.x() + across, bounds.y() + along, depth, length)
            }
            TaskbarPosition::Right => Rect::new(
                bounds.max_x() - across - depth,
                bounds.y() + along,
                depth,
                length,
            ),
        }
    }

    #[must_use]
    pub fn local(self, bounds: Rect, rect: Rect) -> Rect {
        match self.0 {
            TaskbarPosition::Top => Rect::new(
                rect.x() - bounds.x(),
                rect.y() - bounds.y(),
                rect.width(),
                rect.height(),
            ),
            TaskbarPosition::Bottom => Rect::new(
                rect.x() - bounds.x(),
                bounds.max_y() - rect.max_y(),
                rect.width(),
                rect.height(),
            ),
            TaskbarPosition::Left => Rect::new(
                rect.y() - bounds.y(),
                rect.x() - bounds.x(),
                rect.height(),
                rect.width(),
            ),
            TaskbarPosition::Right => Rect::new(
                rect.y() - bounds.y(),
                bounds.max_x() - rect.max_x(),
                rect.height(),
                rect.width(),
            ),
        }
    }

    #[must_use]
    pub fn radii(self, local: CornerRadii) -> CornerRadii {
        let CornerRadii {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        } = local;
        match self.0 {
            TaskbarPosition::Top => local,
            TaskbarPosition::Bottom => {
                CornerRadii::new(bottom_left, bottom_right, top_right, top_left)
            }
            TaskbarPosition::Left => {
                CornerRadii::new(top_left, bottom_left, bottom_right, top_right)
            }
            TaskbarPosition::Right => {
                CornerRadii::new(bottom_left, top_left, top_right, bottom_right)
            }
        }
    }

    #[must_use]
    pub fn inset(self, near: f32, far: f32, start: f32, end: f32) -> Inset {
        match self.0 {
            TaskbarPosition::Top => Inset::exact(near, end, far, start),
            TaskbarPosition::Bottom => Inset::exact(far, end, near, start),
            TaskbarPosition::Left => Inset::exact(start, far, end, near),
            TaskbarPosition::Right => Inset::exact(start, near, end, far),
        }
    }
}
