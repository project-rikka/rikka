use crate::{ROW_HEIGHT, SCROLL_STEP, WINDOW_HEIGHT};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounce {
    offset: f32,
    direction: f32,
    furthest: f32,
}

impl Bounce {
    #[must_use]
    pub fn new(rows: usize) -> Bounce {
        Bounce {
            offset: 0.0,
            direction: 1.0,
            furthest: rows as f32 * ROW_HEIGHT - WINDOW_HEIGHT,
        }
    }

    pub fn advance(&mut self) -> f32 {
        if self.offset + SCROLL_STEP >= self.furthest {
            self.direction = -1.0;
        } else if self.offset - SCROLL_STEP <= 0.0 {
            self.direction = 1.0;
        }
        self.offset = (self.offset + SCROLL_STEP * self.direction).clamp(0.0, self.furthest);
        self.offset
    }

    #[must_use]
    pub fn offset(&self) -> f32 {
        self.offset
    }
}
