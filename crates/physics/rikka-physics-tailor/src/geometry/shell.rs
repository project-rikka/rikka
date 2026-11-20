use std::collections::HashMap;

use glam::{Vec2, Vec3};

use super::{Feature, TriangleTree, doubled_area_normal};

#[derive(Clone, Debug)]
pub(crate) struct Shell {
    tree: TriangleTree,
    triangles: Vec<[u32; 3]>,
    faces: Vec<Vec3>,
    corners: Vec<Vec3>,
    sides: HashMap<(u32, u32), Vec3>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Signed {
    pub(crate) distance: f32,
    pub(crate) triangle: u32,
    pub(crate) uv: Vec2,
    pub(crate) outward: Vec3,
}

impl Shell {
    pub(crate) fn new(positions: &[Vec3], triangles: &[[u32; 3]]) -> Shell {
        let corners_of = |triangle: &[u32; 3]| triangle.map(|corner| positions[corner as usize]);
        let faces: Vec<Vec3> = triangles
            .iter()
            .map(|triangle| doubled_area_normal(corners_of(triangle)).normalize_or_zero())
            .collect();
        let mut corners = vec![Vec3::ZERO; positions.len()];
        let mut sides: HashMap<(u32, u32), Vec3> = HashMap::new();
        for (triangle, face) in triangles.iter().zip(&faces) {
            let at = corners_of(triangle);
            for corner in 0..3 {
                let (from, to) = (
                    at[(corner + 1) % 3] - at[corner],
                    at[(corner + 2) % 3] - at[corner],
                );
                let angle = from.angle_between(to);
                if angle.is_finite() {
                    corners[triangle[corner] as usize] += *face * angle;
                }
                let (a, b) = (triangle[corner], triangle[(corner + 1) % 3]);
                *sides.entry((a.min(b), a.max(b))).or_insert(Vec3::ZERO) += *face;
            }
        }
        Shell {
            tree: TriangleTree::new(triangles.iter().map(corners_of).collect()),
            triangles: triangles.to_vec(),
            faces,
            corners,
            sides,
        }
    }

    pub(crate) fn signed(&self, point: Vec3, limit: f32) -> Option<Signed> {
        let hit = self.tree.nearest(point, limit)?;
        let triangle = self.triangles[hit.triangle as usize];
        let pseudo = match hit.closest.feature {
            Feature::Face => self.faces[hit.triangle as usize],
            Feature::Corner(corner) => self.corners[triangle[corner] as usize],
            Feature::Edge(side) => {
                let (a, b) = (triangle[side], triangle[(side + 1) % 3]);
                self.sides
                    .get(&(a.min(b), a.max(b)))
                    .copied()
                    .unwrap_or(self.faces[hit.triangle as usize])
            }
        };
        let away = point - hit.closest.point;
        let sign = if away.dot(pseudo) < 0.0 { -1.0 } else { 1.0 };
        let outward = if away.length_squared() > 1e-14 {
            away.normalize() * sign
        } else {
            pseudo.normalize_or(self.faces[hit.triangle as usize])
        };
        Some(Signed {
            distance: hit.distance * sign,
            triangle: hit.triangle,
            uv: hit.closest.uv,
            outward,
        })
    }

    pub(crate) fn corners(&self, triangle: u32) -> [Vec3; 3] {
        self.tree.corners(triangle)
    }
}
