use glam::{Mat4, Vec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl Default for Bounds {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl Bounds {
    pub const EMPTY: Self = Self {
        min: Vec3::splat(f32::INFINITY),
        max: Vec3::splat(f32::NEG_INFINITY),
    };

    #[must_use]
    pub fn from_points(points: impl IntoIterator<Item = Vec3>) -> Self {
        let mut bounds = Self::EMPTY;
        for point in points {
            bounds.grow(point);
        }
        bounds
    }

    pub fn grow(&mut self, point: Vec3) {
        self.min = self.min.min(point);
        self.max = self.max.max(point);
    }

    #[must_use]
    pub fn union(self, other: Self) -> Self {
        match (self.is_empty(), other.is_empty()) {
            (true, _) => other,
            (_, true) => self,
            _ => Self {
                min: self.min.min(other.min),
                max: self.max.max(other.max),
            },
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.min.cmpgt(self.max).any()
    }

    #[must_use]
    pub fn center(&self) -> Vec3 {
        match self.is_empty() {
            true => Vec3::ZERO,
            false => (self.min + self.max) * 0.5,
        }
    }

    #[must_use]
    pub fn size(&self) -> Vec3 {
        match self.is_empty() {
            true => Vec3::ZERO,
            false => self.max - self.min,
        }
    }

    #[must_use]
    pub fn longest(&self) -> f32 {
        self.size().max_element()
    }

    #[must_use]
    pub fn radius(&self) -> f32 {
        self.size().length() * 0.5
    }

    #[must_use]
    pub fn overlaps(&self, other: &Self, margin: f32) -> bool {
        !self.is_empty()
            && !other.is_empty()
            && (self.min - margin).cmple(other.max).all()
            && (other.min - margin).cmple(self.max).all()
    }

    #[must_use]
    pub fn transformed(self, matrix: Mat4) -> Self {
        if self.is_empty() {
            return self;
        }
        let corner = |x: f32, y: f32, z: f32| matrix.transform_point3(Vec3::new(x, y, z));
        Self::from_points([
            corner(self.min.x, self.min.y, self.min.z),
            corner(self.max.x, self.min.y, self.min.z),
            corner(self.min.x, self.max.y, self.min.z),
            corner(self.max.x, self.max.y, self.min.z),
            corner(self.min.x, self.min.y, self.max.z),
            corner(self.max.x, self.min.y, self.max.z),
            corner(self.min.x, self.max.y, self.max.z),
            corner(self.max.x, self.max.y, self.max.z),
        ])
    }
}
