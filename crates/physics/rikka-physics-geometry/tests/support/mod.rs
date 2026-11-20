use rikka_physics_geometry::ConvexHull;
use rikka_physics_math::{Quat, Transform, Vec3};

pub(crate) struct Random(u64);

impl Random {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1)
    }

    pub(crate) fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }

    pub(crate) fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.next()
    }

    pub(crate) fn vector(&mut self, extent: f32) -> Vec3 {
        Vec3::new(
            self.range(-extent, extent),
            self.range(-extent, extent),
            self.range(-extent, extent),
        )
    }

    pub(crate) fn rotation(&mut self) -> Quat {
        let axis = self.vector(1.0).normalize_or(Vec3::Y);
        Quat::from_axis_angle(axis, self.range(-3.1, 3.1))
    }

    pub(crate) fn transform(&mut self, extent: f32) -> Transform {
        Transform::new(self.vector(extent), self.rotation())
    }
}

pub(crate) fn close(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance * (1.0 + a.abs().max(b.abs()))
}

pub(crate) fn close_vec(a: Vec3, b: Vec3, tolerance: f32) -> bool {
    (a - b).length() <= tolerance * (1.0 + a.length().max(b.length()))
}

pub(crate) fn ray_triangle(
    origin: Vec3,
    translation: Vec3,
    [a, b, c]: [Vec3; 3],
    max_fraction: f32,
) -> Option<f32> {
    let ab = b - a;
    let ac = c - a;
    let p = translation.cross(ac);
    let determinant = ab.dot(p);
    if determinant <= 0.0 {
        return None;
    }
    let inverse = 1.0 / determinant;
    let offset = origin - a;
    let u = offset.dot(p) * inverse;
    let q = offset.cross(ab);
    let v = translation.dot(q) * inverse;
    if !(0.0..=1.0).contains(&u) || v < 0.0 || u + v > 1.0 {
        return None;
    }
    let fraction = ac.dot(q) * inverse;
    (0.0..=max_fraction).contains(&fraction).then_some(fraction)
}

pub(crate) fn face_rings(hull: &ConvexHull) -> Vec<Vec<usize>> {
    let edges = hull.edges();
    hull.blob()
        .section(hull.header().faces)
        .iter()
        .map(|&first| {
            let first = usize::from(first);
            let mut ring = vec![first];
            let mut edge = usize::from(edges[first].next);
            while edge != first {
                ring.push(edge);
                edge = usize::from(edges[edge].next);
            }
            ring
        })
        .collect()
}
