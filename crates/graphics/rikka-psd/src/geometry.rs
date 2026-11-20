#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    pub top: i32,
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
}

impl Rect {
    pub const EMPTY: Rect = Rect {
        top: 0,
        left: 0,
        bottom: 0,
        right: 0,
    };

    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            top,
            left,
            bottom,
            right,
        }
    }

    pub const fn from_size(left: i32, top: i32, width: u32, height: u32) -> Self {
        Self {
            top,
            left,
            bottom: top + height as i32,
            right: left + width as i32,
        }
    }

    pub fn width(&self) -> u32 {
        (i64::from(self.right) - i64::from(self.left)).max(0) as u32
    }

    pub fn height(&self) -> u32 {
        (i64::from(self.bottom) - i64::from(self.top)).max(0) as u32
    }

    pub fn is_empty(&self) -> bool {
        self.width() == 0 || self.height() == 0
    }

    pub fn area(&self) -> u64 {
        u64::from(self.width()) * u64::from(self.height())
    }

    pub fn intersection(&self, other: &Rect) -> Rect {
        let rect = Rect {
            top: self.top.max(other.top),
            left: self.left.max(other.left),
            bottom: self.bottom.min(other.bottom),
            right: self.right.min(other.right),
        };
        if rect.is_empty() { Rect::EMPTY } else { rect }
    }

    pub fn union(&self, other: &Rect) -> Rect {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        Rect {
            top: self.top.min(other.top),
            left: self.left.min(other.left),
            bottom: self.bottom.max(other.bottom),
            right: self.right.max(other.right),
        }
    }

    pub fn translated(&self, dx: i32, dy: i32) -> Rect {
        Rect {
            top: self.top + dy,
            left: self.left + dx,
            bottom: self.bottom + dy,
            right: self.right + dx,
        }
    }

    pub fn expanded(&self, amount: i32) -> Rect {
        Rect {
            top: self.top.saturating_sub(amount),
            left: self.left.saturating_sub(amount),
            bottom: self.bottom.saturating_add(amount),
            right: self.right.saturating_add(amount),
        }
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }

    pub fn encloses(&self, other: &Rect) -> bool {
        !other.is_empty() && self.intersection(other) == *other
    }
}
