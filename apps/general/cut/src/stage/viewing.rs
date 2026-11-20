use cut_project::Format;
use rikka_app::prelude::*;

const MARGIN: f32 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Viewing {
    pub(crate) frame: Rect,
    pub(crate) scale: f32,
}

fn fitted(room: Rect, width: f32, height: f32) -> Rect {
    if width <= 0.0 || height <= 0.0 || room.width() <= 0.0 || room.height() <= 0.0 {
        return Rect::new(room.center().x, room.center().y, 0.0, 0.0);
    }
    let scale = (room.width() / width).min(room.height() / height);
    let (shown_width, shown_height) = (width * scale, height * scale);
    Rect::new(
        room.center().x - shown_width / 2.0,
        room.center().y - shown_height / 2.0,
        shown_width,
        shown_height,
    )
}

impl Viewing {
    pub(crate) fn new(bounds: Rect, format: Format) -> Viewing {
        let room = Rect::new(
            bounds.x() + MARGIN,
            bounds.y() + MARGIN,
            (bounds.width() - MARGIN * 2.0).max(0.0),
            (bounds.height() - MARGIN * 2.0).max(0.0),
        );
        let frame = fitted(room, format.width as f32, format.height as f32);
        Viewing {
            frame,
            scale: match format.width {
                0 => 1.0,
                width => frame.width() / width as f32,
            },
        }
    }

    pub(crate) fn to_screen(self, at: Point) -> Point {
        Point::new(
            self.frame.x() + at.x * self.scale,
            self.frame.y() + at.y * self.scale,
        )
    }

    pub(crate) fn to_stage(self, at: Point) -> Point {
        let scale = self.scale.max(f32::EPSILON);
        Point::new(
            (at.x - self.frame.x()) / scale,
            (at.y - self.frame.y()) / scale,
        )
    }
}
