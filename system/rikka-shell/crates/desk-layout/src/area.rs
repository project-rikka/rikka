use rikka_types::{Rect, Size as Extent};
use serde::{Deserialize, Serialize, Serializer};

use crate::widget::Size;

pub const MARGIN: f32 = 12.0;

pub const GAP: f32 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spot {
    #[serde(serialize_with = "tidy")]
    pub x: f32,
    #[serde(serialize_with = "tidy")]
    pub y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    room: Rect,
}

impl Spot {
    pub const CENTER: Spot = Spot { x: 0.5, y: 0.5 };

    #[must_use]
    pub fn new(x: f32, y: f32) -> Spot {
        Spot { x, y }.sanitized()
    }

    pub(super) fn sanitized(self) -> Spot {
        let unit = |value: f32| match value.is_finite() {
            true => value.clamp(0.0, 1.0),
            false => 0.5,
        };
        Spot {
            x: unit(self.x),
            y: unit(self.y),
        }
    }
}

impl Area {
    #[must_use]
    pub fn for_desk(desk: Rect) -> Area {
        let inset = MARGIN
            .min(desk.width().max(0.0) / 2.0)
            .min(desk.height().max(0.0) / 2.0);
        Area {
            room: Rect::new(
                desk.min_x() + inset,
                desk.min_y() + inset,
                (desk.width() - inset * 2.0).max(0.0),
                (desk.height() - inset * 2.0).max(0.0),
            ),
        }
    }

    #[must_use]
    pub fn room(&self) -> Rect {
        self.room
    }

    #[must_use]
    pub fn place(&self, size: Size, spot: Spot) -> Rect {
        self.place_extent(size.extent(), spot)
    }

    #[must_use]
    pub fn place_extent(&self, extent: Extent, spot: Spot) -> Rect {
        let spot = spot.sanitized();
        let x = (spot.x * self.slack_x(extent.width)).round();
        let y = (spot.y * self.slack_y(extent.height)).round();
        Rect::new(
            self.room.min_x() + x,
            self.room.min_y() + y,
            extent.width,
            extent.height,
        )
    }

    #[must_use]
    pub fn spot_of(&self, rect: Rect) -> Spot {
        let along = |offset: f32, slack: f32| match slack > 0.0 {
            true => offset / slack,
            false => 0.0,
        };
        Spot::new(
            along(rect.min_x() - self.room.min_x(), self.slack_x(rect.width())),
            along(
                rect.min_y() - self.room.min_y(),
                self.slack_y(rect.height()),
            ),
        )
    }

    #[must_use]
    pub fn contain(&self, rect: Rect) -> Rect {
        let fit = |at: f32, start: f32, slack: f32| (at - start).clamp(0.0, slack) + start;
        Rect::new(
            fit(rect.min_x(), self.room.min_x(), self.slack_x(rect.width())),
            fit(rect.min_y(), self.room.min_y(), self.slack_y(rect.height())),
            rect.width(),
            rect.height(),
        )
    }

    fn slack_x(&self, width: f32) -> f32 {
        (self.room.width() - width).max(0.0)
    }

    fn slack_y(&self, height: f32) -> f32 {
        (self.room.height() - height).max(0.0)
    }
}

fn tidy<S: Serializer>(value: &f32, serializer: S) -> Result<S::Ok, S::Error> {
    let millionths = (f64::from(*value) * 1e6).round();
    serializer.serialize_f64(millionths / 1e6)
}
