#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bounds {
    pub x_min: f32,
    pub y_min: f32,
    pub x_max: f32,
    pub y_max: f32,
}

impl Bounds {
    pub const EMPTY: Self = Self {
        x_min: f32::INFINITY,
        y_min: f32::INFINITY,
        x_max: f32::NEG_INFINITY,
        y_max: f32::NEG_INFINITY,
    };

    #[inline]
    pub fn is_empty(&self) -> bool {
        !(self.x_min <= self.x_max && self.y_min <= self.y_max)
    }

    #[inline]
    pub fn width(&self) -> f32 {
        if self.is_empty() {
            0.0
        } else {
            self.x_max - self.x_min
        }
    }

    #[inline]
    pub fn height(&self) -> f32 {
        if self.is_empty() {
            0.0
        } else {
            self.y_max - self.y_min
        }
    }

    #[inline]
    pub fn include(&mut self, x: f32, y: f32) {
        self.x_min = self.x_min.min(x);
        self.y_min = self.y_min.min(y);
        self.x_max = self.x_max.max(x);
        self.y_max = self.y_max.max(y);
    }

    #[inline]
    pub fn union(&self, other: &Self) -> Self {
        Self {
            x_min: self.x_min.min(other.x_min),
            y_min: self.y_min.min(other.y_min),
            x_max: self.x_max.max(other.x_max),
            y_max: self.y_max.max(other.y_max),
        }
    }

    #[inline]
    pub fn intersection(&self, other: &Self) -> Self {
        Self {
            x_min: self.x_min.max(other.x_min),
            y_min: self.y_min.max(other.y_min),
            x_max: self.x_max.min(other.x_max),
            y_max: self.y_max.min(other.y_max),
        }
    }
}
