use glam::{Vec3, Vec4};

const CORNERS: usize = 8;

const FACES: [[usize; 3]; 6] = [
    [0, 1, 2],
    [4, 6, 5],
    [0, 4, 1],
    [1, 5, 2],
    [2, 6, 3],
    [3, 7, 0],
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipBounds {
    pub corners: [Vec3; CORNERS],
    pub color: Vec3,
}

impl ClipBounds {
    pub(crate) fn inward_planes(&self) -> [Vec4; 6] {
        let center = self.corners.iter().copied().sum::<Vec3>() / CORNERS as f32;
        FACES.map(|[a, b, c]| {
            let (a, b, c) = (self.corners[a], self.corners[b], self.corners[c]);
            let normal = (b - a).cross(c - a).normalize_or_zero();
            let plane = normal.extend(-normal.dot(a));
            match plane.truncate().dot(center) + plane.w < 0.0 {
                true => -plane,
                false => plane,
            }
        })
    }
}
