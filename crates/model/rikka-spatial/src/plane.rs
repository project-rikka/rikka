use glam::{Vec3, Vec4};
use smallvec::SmallVec;

use crate::aabb::Aabb;
use crate::polygon::polygon_normal;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    pub normal: Vec3,
    pub d: f32,
}

impl Plane {
    pub const fn new(normal: Vec3, d: f32) -> Self {
        Self { normal, d }
    }

    pub fn from_point_normal(point: Vec3, normal: Vec3) -> Option<Self> {
        let normal = normal.try_normalize()?;
        Some(Self::new(normal, -normal.dot(point)))
    }

    pub fn from_points(a: Vec3, b: Vec3, c: Vec3) -> Option<Self> {
        Self::from_point_normal(a, (b - a).cross(c - a))
    }

    pub fn from_polygon(polygon: &[Vec3]) -> Option<Self> {
        if polygon.len() < 3 {
            return None;
        }
        let normal = polygon_normal(polygon).try_normalize()?;
        let centroid = polygon.iter().copied().sum::<Vec3>() / polygon.len() as f32;
        Some(Self::new(normal, -normal.dot(centroid)))
    }

    pub fn from_vec4(coefficients: Vec4) -> Option<Self> {
        let length = coefficients.truncate().length();
        (length > 0.0 && length.is_finite())
            .then(|| Self::new(coefficients.truncate() / length, coefficients.w / length))
    }

    pub fn to_vec4(&self) -> Vec4 {
        self.normal.extend(self.d)
    }

    pub fn signed_distance(&self, point: Vec3) -> f32 {
        self.normal.dot(point) + self.d
    }

    pub fn project(&self, point: Vec3) -> Vec3 {
        point - self.normal * self.signed_distance(point)
    }

    #[must_use]
    pub fn flipped(&self) -> Self {
        Self::new(-self.normal, -self.d)
    }

    pub fn classify_aabb(&self, bounds: &Aabb) -> Containment {
        let ascending = self.normal.cmpge(Vec3::ZERO);
        let farthest = Vec3::select(ascending, bounds.max, bounds.min);
        let nearest = Vec3::select(ascending, bounds.min, bounds.max);
        if self.signed_distance(farthest) < 0.0 {
            Containment::Outside
        } else if self.signed_distance(nearest) >= 0.0 {
            Containment::Inside
        } else {
            Containment::Intersecting
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Containment {
    Outside,
    Intersecting,
    Inside,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Frustum {
    planes: SmallVec<[Plane; 6]>,
}

impl Frustum {
    pub fn new(planes: impl IntoIterator<Item = Plane>) -> Self {
        Self {
            planes: planes.into_iter().collect(),
        }
    }

    pub fn planes(&self) -> &[Plane] {
        &self.planes
    }

    pub fn contains_point(&self, point: Vec3) -> bool {
        self.planes
            .iter()
            .all(|plane| plane.signed_distance(point) >= 0.0)
    }

    pub fn classify_aabb(&self, bounds: &Aabb) -> Containment {
        let mut containment = Containment::Inside;
        for plane in &self.planes {
            match plane.classify_aabb(bounds) {
                Containment::Outside => return Containment::Outside,
                Containment::Intersecting => containment = Containment::Intersecting,
                Containment::Inside => {}
            }
        }
        containment
    }

    pub fn classify_sphere(&self, center: Vec3, radius: f32) -> Containment {
        let mut containment = Containment::Inside;
        for plane in &self.planes {
            let distance = plane.signed_distance(center);
            if distance < -radius {
                return Containment::Outside;
            }
            if distance < radius {
                containment = Containment::Intersecting;
            }
        }
        containment
    }

    pub fn classify_segment(&self, start: Vec3, end: Vec3) -> Containment {
        self.classify_polygon(&[start, end])
    }

    pub fn classify_triangle(&self, triangle: [Vec3; 3]) -> Containment {
        self.classify_polygon(&triangle)
    }

    pub fn classify_polygon(&self, polygon: &[Vec3]) -> Containment {
        if polygon.is_empty() {
            return Containment::Outside;
        }
        if polygon.iter().all(|&point| self.contains_point(point)) {
            return Containment::Inside;
        }
        let mut current: SmallVec<[Vec3; 16]> = polygon.iter().copied().collect();
        let mut next: SmallVec<[Vec3; 16]> = SmallVec::new();
        for plane in &self.planes {
            next.clear();
            clip_polygon(plane, &current, &mut next);
            if next.is_empty() {
                return Containment::Outside;
            }
            std::mem::swap(&mut current, &mut next);
        }
        Containment::Intersecting
    }
}

fn clip_polygon(plane: &Plane, polygon: &[Vec3], clipped: &mut SmallVec<[Vec3; 16]>) {
    let count = polygon.len();
    let closed = count > 2;
    let edges = if closed { count } else { count - 1 };
    if count == 1 {
        if plane.signed_distance(polygon[0]) >= 0.0 {
            clipped.push(polygon[0]);
        }
        return;
    }
    for index in 0..edges {
        let start = polygon[index];
        let end = polygon[(index + 1) % count];
        let start_distance = plane.signed_distance(start);
        let end_distance = plane.signed_distance(end);
        let start_inside = start_distance >= 0.0;
        let end_inside = end_distance >= 0.0;
        if start_inside {
            clipped.push(start);
        }
        if start_inside != end_inside {
            let factor = start_distance / (start_distance - end_distance);
            clipped.push(start.lerp(end, factor.clamp(0.0, 1.0)));
        }
        if !closed && end_inside {
            clipped.push(end);
        }
    }
}
