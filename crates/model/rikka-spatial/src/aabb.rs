use glam::{Affine3A, Vec3, Vec3A};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub const EMPTY: Aabb = Aabb {
        min: Vec3::splat(f32::INFINITY),
        max: Vec3::splat(f32::NEG_INFINITY),
    };

    pub const fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn from_point(point: Vec3) -> Self {
        Self::new(point, point)
    }

    pub fn from_points(points: impl IntoIterator<Item = Vec3>) -> Self {
        points
            .into_iter()
            .fold(Self::EMPTY, |bounds, point| bounds.with_point(point))
    }

    pub fn from_triangle(triangle: [Vec3; 3]) -> Self {
        let [a, b, c] = triangle;
        Self::new(a.min(b).min(c), a.max(b).max(c))
    }

    pub fn from_segment(start: Vec3, end: Vec3) -> Self {
        Self::new(start.min(end), start.max(end))
    }

    pub fn from_sphere(center: Vec3, radius: f32) -> Self {
        Self::new(center - Vec3::splat(radius), center + Vec3::splat(radius))
    }

    pub fn is_empty(&self) -> bool {
        !(self.min.x <= self.max.x && self.min.y <= self.max.y && self.min.z <= self.max.z)
    }

    #[must_use]
    pub fn with_point(&self, point: Vec3) -> Self {
        Self::new(self.min.min(point), self.max.max(point))
    }

    #[must_use]
    pub fn union(&self, other: &Aabb) -> Self {
        Self::new(self.min.min(other.min), self.max.max(other.max))
    }

    #[must_use]
    pub fn intersection(&self, other: &Aabb) -> Self {
        Self::new(self.min.max(other.min), self.max.min(other.max))
    }

    #[must_use]
    pub fn expanded(&self, margin: f32) -> Self {
        Self::new(
            self.min - Vec3::splat(margin),
            self.max + Vec3::splat(margin),
        )
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    pub fn surface_area(&self) -> f32 {
        if self.is_empty() {
            return 0.0;
        }
        let size = self.size();
        2.0 * (size.x * size.y + size.y * size.z + size.z * size.x)
    }

    pub fn corners(&self) -> [Vec3; 8] {
        let (min, max) = (self.min, self.max);
        [
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(min.x, max.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(min.x, max.y, max.z),
            Vec3::new(max.x, max.y, max.z),
        ]
    }

    pub fn contains_point(&self, point: Vec3) -> bool {
        point.cmpge(self.min).all() && point.cmple(self.max).all()
    }

    pub fn contains(&self, other: &Aabb) -> bool {
        other.min.cmpge(self.min).all() && other.max.cmple(self.max).all()
    }

    pub fn intersects(&self, other: &Aabb) -> bool {
        self.min.cmple(other.max).all() && other.min.cmple(self.max).all()
    }

    pub fn distance_squared_to_point(&self, point: Vec3) -> f32 {
        let point = Vec3A::from(point);
        let outside = (Vec3A::from(self.min) - point).max(point - Vec3A::from(self.max));
        outside.max(Vec3A::ZERO).length_squared()
    }

    pub fn intersects_sphere(&self, center: Vec3, radius: f32) -> bool {
        self.distance_squared_to_point(center) <= radius * radius
    }

    pub fn closest_point(&self, point: Vec3) -> Vec3 {
        point.clamp(self.min, self.max)
    }

    #[must_use]
    pub fn transformed(&self, transform: &Affine3A) -> Self {
        if self.is_empty() {
            return *self;
        }
        let center = transform.transform_point3a(Vec3A::from(self.center()));
        let half = Vec3A::from(self.size() * 0.5);
        let matrix = transform.matrix3;
        let extent = matrix.x_axis.abs() * half.x
            + matrix.y_axis.abs() * half.y
            + matrix.z_axis.abs() * half.z;
        Self::new(Vec3::from(center - extent), Vec3::from(center + extent))
    }
}

impl Default for Aabb {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl FromIterator<Vec3> for Aabb {
    fn from_iter<I: IntoIterator<Item = Vec3>>(points: I) -> Self {
        Self::from_points(points)
    }
}
