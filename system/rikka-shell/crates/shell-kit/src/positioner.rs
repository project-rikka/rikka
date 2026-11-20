use std::cell::Cell;
use std::rc::Rc;

use rikka_types::{Point, Rect, Size};

use crate::Placement;
use crate::tracked::Tracked;

#[derive(Clone, Debug)]
pub struct Positioner {
    anchor_rect: Tracked<Option<Rect>>,
    gap: f32,
    flipped: Rc<Cell<bool>>,
}

impl Positioner {
    #[must_use]
    pub fn new(gap: f32) -> Positioner {
        Positioner {
            anchor_rect: Tracked::new(None),
            gap,
            flipped: Rc::new(Cell::new(false)),
        }
    }

    pub fn set_anchor_rect(&self, rect: Option<Rect>) {
        self.anchor_rect.set(rect);
    }

    #[must_use]
    pub fn is_flipped(&self) -> bool {
        self.flipped.get()
    }

    #[must_use]
    pub fn geometry_for(&self, placement: &Placement, size: Size) -> Option<Point> {
        let spot = placement.to_surface(self.anchor_rect.get()?);
        let room = placement.size;
        let on_this_output = spot.min_x() >= 0.0
            && spot.min_y() >= 0.0
            && spot.min_x() < room.width
            && spot.min_y() < room.height;
        if !on_this_output {
            return None;
        }
        let below = spot.max_y() + self.gap;
        let fits_below = below + size.height <= room.height;
        self.flipped.set(!fits_below);
        let y = match fits_below {
            true => below,
            false => (spot.min_y() - self.gap - size.height).max(0.0),
        };
        let x = spot.min_x().min(room.width - size.width).max(0.0);
        Some(Point::new(x.round(), y.round()))
    }
}
