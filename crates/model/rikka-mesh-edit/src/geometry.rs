use glam::{Vec2, Vec3};
use rikka_model_core::Bounds;
use rikka_spatial::{FillRule, point_in_polygon};

use crate::ids::{EdgeId, FaceId, LoopId, VertId};
use crate::math::{
    AxisProjection, angle_normalized, angle_three, normalize, normalize_with_length,
};
use crate::mesh::EditMesh;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FaceTangent {
    #[default]
    Auto,
    Edge,
    EdgePair,
    EdgeDiagonal,
    VertDiagonal,
}

impl EditMesh {
    #[must_use]
    pub fn bounds(&self) -> Bounds {
        Bounds::from_points(self.verts().map(|vert| self.position(vert)))
    }

    #[must_use]
    pub fn edge_length(&self, edge: EdgeId) -> f32 {
        let [a, b] = self.edge_verts(edge);
        self.position(a).distance(self.position(b))
    }

    #[must_use]
    pub fn edge_length_squared(&self, edge: EdgeId) -> f32 {
        let [a, b] = self.edge_verts(edge);
        self.position(a).distance_squared(self.position(b))
    }

    #[must_use]
    pub fn edge_center(&self, edge: EdgeId) -> Vec3 {
        let [a, b] = self.edge_verts(edge);
        (self.position(a) + self.position(b)) * 0.5
    }

    #[must_use]
    pub fn edge_face_angle(&self, edge: EdgeId) -> Option<f32> {
        let (a, b) = self.edge_loop_pair(edge)?;
        Some(angle_normalized(
            self.face_normal(self.loop_face(a)),
            self.face_normal(self.loop_face(b)),
        ))
    }

    #[must_use]
    pub fn edge_face_angle_signed(&self, edge: EdgeId) -> Option<f32> {
        let angle = self.edge_face_angle(edge)?;
        Some(match self.is_edge_convex(edge) {
            true => angle,
            false => -angle,
        })
    }

    #[must_use]
    pub fn edge_face_tangent(&self, edge: EdgeId, lp: LoopId) -> Vec3 {
        debug_assert_eq!(self.loop_edge(lp), edge, "loop is not on the edge");
        let along = self.position(self.loop_vert(lp)) - self.position(self.loop_next_vert(lp));
        normalize(along.cross(self.face_normal(self.loop_face(lp))))
    }

    #[must_use]
    pub fn vert_edge_angle(&self, vert: VertId) -> Option<f32> {
        let (a, b) = self.vert_edge_pair(vert)?;
        let first = self.edge_other_vert(a, vert)?;
        let second = self.edge_other_vert(b, vert)?;
        Some(
            std::f32::consts::PI
                - angle_three(
                    self.position(first),
                    self.position(vert),
                    self.position(second),
                ),
        )
    }

    #[must_use]
    pub fn vert_shell_factor(&self, vert: VertId) -> f32 {
        let normal = self.vert_normal(vert);
        let mut shell = 0.0;
        let mut total = 0.0;
        for lp in self.vert_loops(vert) {
            let angle = self.loop_angle(lp);
            shell += shell_distance(normal, self.face_normal(self.loop_face(lp))) * angle;
            total += angle;
        }
        match total != 0.0 {
            true => shell / total,
            false => 1.0,
        }
    }

    #[must_use]
    pub fn loop_angle(&self, lp: LoopId) -> f32 {
        angle_three(
            self.position(self.loop_vert(self.loop_prev(lp))),
            self.position(self.loop_vert(lp)),
            self.position(self.loop_next_vert(lp)),
        )
    }

    #[must_use]
    pub fn loop_normal(&self, lp: LoopId) -> Vec3 {
        let here = self.position(self.loop_vert(lp));
        let before = self.position(self.loop_vert(self.loop_prev(lp))) - here;
        let after = self.position(self.loop_next_vert(lp)) - here;
        let (normal, length) = normalize_with_length(before.cross(after));
        match length == 0.0 {
            true => self.face_normal(self.loop_face(lp)),
            false => normal,
        }
    }

    #[must_use]
    pub fn loop_direction(&self, lp: LoopId) -> Vec3 {
        let here = self.position(self.loop_vert(lp));
        let before = normalize(here - self.position(self.loop_vert(self.loop_prev(lp))));
        let after = normalize(self.position(self.loop_next_vert(lp)) - here);
        normalize(before + after)
    }

    #[must_use]
    pub fn loop_tangent(&self, lp: LoopId) -> Vec3 {
        let here = self.position(self.loop_vert(lp));
        let before = normalize(self.position(self.loop_vert(self.loop_prev(lp))) - here);
        let after = normalize(here - self.position(self.loop_next_vert(lp)));
        let direction = before + after;
        let face_normal = self.face_normal(self.loop_face(lp));
        let tangent = match (before - after).abs().max_element() <= f32::EPSILON * 10.0 {
            false => {
                let mut normal = before.cross(after);
                if normal.dot(face_normal) < 0.0 {
                    normal = -normal;
                }
                direction.cross(normal)
            }
            true => direction.cross(face_normal),
        };
        normalize(tangent)
    }

    #[must_use]
    pub fn face_area(&self, face: FaceId) -> f32 {
        self.face_newell(face).length() * 0.5
    }

    #[must_use]
    pub fn face_perimeter(&self, face: FaceId) -> f32 {
        self.face_loops(face)
            .map(|lp| {
                self.position(self.loop_vert(lp))
                    .distance(self.position(self.loop_next_vert(lp)))
            })
            .sum()
    }

    #[must_use]
    pub fn face_center_median(&self, face: FaceId) -> Vec3 {
        let mut sum = Vec3::ZERO;
        let mut count = 0;
        for vert in self.face_verts(face) {
            sum += self.position(vert);
            count += 1;
        }
        sum / count.max(1) as f32
    }

    #[must_use]
    pub fn face_center_bounds(&self, face: FaceId) -> Vec3 {
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for vert in self.face_verts(face) {
            min = min.min(self.position(vert));
            max = max.max(self.position(vert));
        }
        (min + max) * 0.5
    }

    #[must_use]
    pub fn face_center_median_weighted(&self, face: FaceId) -> Vec3 {
        let first = self.face_first_loop(face);
        let mut previous_length = self
            .position(self.loop_vert(self.loop_prev(first)))
            .distance(self.position(self.loop_vert(first)));
        let mut sum = Vec3::ZERO;
        let mut total = 0.0;
        for lp in self.face_loops(face) {
            let here = self.position(self.loop_vert(lp));
            let length = here.distance(self.position(self.loop_next_vert(lp)));
            let weight = length + previous_length;
            sum += here * weight;
            total += weight;
            previous_length = length;
        }
        match total != 0.0 {
            true => sum / total,
            false => sum,
        }
    }

    #[must_use]
    pub fn face_longest_loop(&self, face: FaceId) -> LoopId {
        let mut best = self.face_first_loop(face);
        let mut longest = 0.0;
        for lp in self.face_loops(face) {
            let length = self
                .position(self.loop_vert(lp))
                .distance_squared(self.position(self.loop_next_vert(lp)));
            if length >= longest {
                best = lp;
                longest = length;
            }
        }
        best
    }

    #[must_use]
    pub fn face_tangent(&self, face: FaceId, method: FaceTangent) -> Vec3 {
        match method {
            FaceTangent::Auto => self.calc_face_tangent_auto(face),
            FaceTangent::Edge => self.tangent_from_edge(face),
            FaceTangent::EdgePair => self.tangent_from_edge_pair(face),
            FaceTangent::EdgeDiagonal => self.tangent_from_edge_diagonal(face),
            FaceTangent::VertDiagonal => self.tangent_from_vert_diagonal(face),
        }
    }

    pub(crate) fn calc_face_tangent_auto(&self, face: FaceId) -> Vec3 {
        match self.face_len(face) {
            3 => {
                let verts = self.face_verts_vec(face);
                let index = self.unique_triangle_edge(&verts);
                normalize(self.position(verts[index]) - self.position(verts[(index + 1) % 3]))
            }
            4 => self.quad_edge_pair_tangent(face),
            _ => self.tangent_from_edge(face),
        }
    }

    fn unique_triangle_edge(&self, verts: &[VertId]) -> usize {
        let at = |index: usize| self.position(verts[index]);
        let lengths = [
            at(0).distance(at(1)),
            at(1).distance(at(2)),
            at(2).distance(at(0)),
        ];
        let differences = [
            (lengths[1] - lengths[2]).abs(),
            (lengths[2] - lengths[0]).abs(),
            (lengths[0] - lengths[1]).abs(),
        ];
        let mut order = [0usize, 1, 2];
        let swap = |order: &mut [usize; 3], a: usize, b: usize| {
            if differences[order[a]] > differences[order[b]] {
                order.swap(a, b);
            }
        };
        swap(&mut order, 0, 1);
        swap(&mut order, 1, 2);
        swap(&mut order, 0, 1);
        order[0]
    }

    fn tangent_from_edge(&self, face: FaceId) -> Vec3 {
        let longest = self.face_longest_loop(face);
        normalize(
            self.position(self.loop_vert(longest)) - self.position(self.loop_next_vert(longest)),
        )
    }

    fn quad_edge_pair_tangent(&self, face: FaceId) -> Vec3 {
        let verts = self.face_verts_vec(face);
        let at = |index: usize| self.position(verts[index]);
        let first = (at(3) - at(2)) + (at(0) - at(1));
        let second = (at(0) - at(3)) + (at(1) - at(2));
        match first.length_squared() < second.length_squared() {
            true => normalize(second),
            false => normalize(first),
        }
    }

    fn tangent_from_edge_pair(&self, face: FaceId) -> Vec3 {
        match self.face_len(face) {
            3 => {
                let verts = self.face_verts_vec(face);
                let index = self.unique_triangle_edge(&verts);
                let a = self.position(verts[index]);
                let b = self.position(verts[(index + 1) % 3]);
                let other = self.position(verts[(index + 2) % 3]);
                normalize(other - (a + b) * 0.5)
            }
            4 => self.quad_edge_pair_tangent(face),
            _ => {
                let longest = self.face_longest_loop(face);
                let mut other = longest;
                let mut longest_other = 0.0;
                let mut lp = self.loop_prev(self.loop_prev(longest));
                let last = self.loop_next(longest);
                while lp != last {
                    let length = self
                        .position(self.loop_vert(lp))
                        .distance_squared(self.position(self.loop_next_vert(lp)));
                    if length >= longest_other {
                        other = lp;
                        longest_other = length;
                    }
                    lp = self.loop_prev(lp);
                }
                let first = self.position(self.loop_next_vert(longest))
                    - self.position(self.loop_vert(longest));
                let second = self.position(self.loop_vert(other))
                    - self.position(self.loop_next_vert(other));
                let tangent = normalize(first + second);
                match tangent == Vec3::ZERO {
                    true => normalize(first),
                    false => tangent,
                }
            }
        }
    }

    fn tangent_from_edge_diagonal(&self, face: FaceId) -> Vec3 {
        let loops = self.face_loops_vec(face);
        let mut tangent = Vec3::ZERO;
        let mut farthest = 0.0;
        for &lp in &loops {
            let here = self.position(self.loop_vert(lp));
            let mut other = self.loop_next(lp);
            let last = self.loop_prev(lp);
            while other != last {
                let start = self.position(self.loop_vert(other));
                let end = self.position(self.loop_next_vert(other));
                let factor = crate::math::line_point_factor(here, start, end).clamp(0.0, 1.0);
                let offset = here - start.lerp(end, factor);
                if offset.length_squared() > farthest {
                    farthest = offset.length_squared();
                    tangent = offset;
                }
                other = self.loop_next(other);
            }
        }
        normalize(tangent)
    }

    fn tangent_from_vert_diagonal(&self, face: FaceId) -> Vec3 {
        let loops = self.face_loops_vec(face);
        let mut tangent = Vec3::ZERO;
        let mut farthest = 0.0;
        for &lp in &loops {
            let here = self.position(self.loop_vert(lp));
            let mut other = self.loop_next(lp);
            while other != lp {
                let offset = here - self.position(self.loop_vert(other));
                if offset.length_squared() > farthest {
                    farthest = offset.length_squared();
                    tangent = offset;
                }
                other = self.loop_next(other);
            }
        }
        normalize(tangent)
    }

    #[must_use]
    pub fn is_point_in_face(&self, face: FaceId, point: Vec3) -> bool {
        let projection = AxisProjection::new(self.face_normal(face));
        let polygon: Vec<Vec2> = self
            .face_verts(face)
            .map(|vert| projection.project(self.position(vert)))
            .collect();
        point_in_polygon(projection.project(point), &polygon, FillRule::EvenOdd)
    }

    #[must_use]
    pub fn is_face_convex(&self, face: FaceId) -> bool {
        let normal = self.calc_face_normal(face);
        self.face_loops(face).all(|lp| {
            let here = self.position(self.loop_vert(lp));
            let before = here - self.position(self.loop_vert(self.loop_prev(lp)));
            let after = self.position(self.loop_next_vert(lp)) - here;
            before.cross(after).dot(normal) >= 0.0
        })
    }

    #[must_use]
    pub fn is_face_degenerate(&self, face: FaceId) -> bool {
        self.calc_face_normal(face) == Vec3::ZERO
    }

    #[must_use]
    pub fn volume(&self, signed: bool) -> f64 {
        let mut volume = 0.0f64;
        let mut triangles = Vec::new();
        for face in self.faces() {
            triangles.clear();
            self.face_triangles(face, false, &mut triangles);
            for triangle in &triangles {
                let [a, b, c] = triangle.map(|lp| self.position(self.loop_vert(lp)).as_dvec3());
                volume += a.dot(b.cross(c));
            }
        }
        volume /= 6.0;
        match signed {
            true => volume,
            false => volume.abs(),
        }
    }
}

const SMALL_NUMBER: f32 = 1.0e-8;

fn shell_distance(a: Vec3, b: Vec3) -> f32 {
    let cosine = a.dot(b).abs();
    match cosine < SMALL_NUMBER {
        true => 1.0,
        false => 1.0 / cosine,
    }
}
