mod highlight;
mod pane;
mod split;
mod split_ratios;

use rikka_types::{Point, Rect};

use rikka_ui::Axis;

pub use pane::{Pane, pane};
pub use split::{SPLIT_GUTTER, Split, split};
pub use split_ratios::{Ratio, SplitRatios};

const EDGE: f32 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    #[must_use]
    pub fn axis(self) -> Axis {
        match self {
            Self::Left | Self::Right => Axis::Horizontal,
            Self::Top | Self::Bottom => Axis::Vertical,
        }
    }

    #[must_use]
    pub fn after(self) -> bool {
        matches!(self, Self::Right | Self::Bottom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropTarget {
    Into,
    Split(Side),
}

fn landing_at(bounds: Rect, point: Point) -> DropTarget {
    let (width, height) = (bounds.width(), bounds.height());
    if width <= 0.0 || height <= 0.0 {
        return DropTarget::Into;
    }
    let reaches = [
        (Side::Left, (point.x - bounds.min_x()) / width),
        (Side::Right, (bounds.max_x() - point.x) / width),
        (Side::Top, (point.y - bounds.min_y()) / height),
        (Side::Bottom, (bounds.max_y() - point.y) / height),
    ];
    reaches
        .into_iter()
        .filter(|(_, reach)| *reach < EDGE)
        .min_by(|(_, near), (_, far)| near.total_cmp(far))
        .map_or(DropTarget::Into, |(side, _)| DropTarget::Split(side))
}

fn region(bounds: Rect, landing: DropTarget) -> Rect {
    let DropTarget::Split(side) = landing else {
        return bounds;
    };
    let (width, height) = (bounds.width() / 2.0, bounds.height() / 2.0);
    match side {
        Side::Left => Rect::new(bounds.x(), bounds.y(), width, bounds.height()),
        Side::Right => Rect::new(bounds.center().x, bounds.y(), width, bounds.height()),
        Side::Top => Rect::new(bounds.x(), bounds.y(), bounds.width(), height),
        Side::Bottom => Rect::new(bounds.x(), bounds.center().y, bounds.width(), height),
    }
}
