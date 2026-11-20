use std::f32::consts::PI;

use glam::Vec3;
use rikka_mesh_edit::{EdgeId, FaceId, VertId};

use crate::math::{
    EPSILON_ANG, EPSILON_BIG, EPSILON_D, LineMeet, Plane, SMALL_ANG, angle_normalized,
    angle_vectors, line_line, midpoint, nearly_parallel_normalized, normalize,
    normalize_with_length,
};
use crate::model::HalfId;
use crate::options::WidthType;
use crate::state::Bevel;

const GOOD_ANGLE: f32 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AngleKind {
    Smaller,
    Straight,
    Larger,
}

pub(crate) struct EdgeMeet {
    pub(crate) point: Option<Vec3>,
    pub(crate) angle: f32,
}

pub(crate) struct BetweenMeet {
    pub(crate) point: Vec3,
    pub(crate) sinratio: Option<f32>,
}

impl Bevel<'_> {
    pub(crate) fn other_vert(&self, edge: EdgeId, vert: VertId) -> VertId {
        self.mesh
            .edge_other_vert(edge, vert)
            .expect("bevel edge touches its vertex")
    }

    pub(crate) fn slide_dist(&self, edge: EdgeId, vert: VertId, distance: f32) -> Vec3 {
        let here = self.position(vert);
        let (direction, length) =
            normalize_with_length(here - self.position(self.other_vert(edge, vert)));
        let distance = match distance > length {
            true => length - (50.0 * EPSILON_D) as f32,
            false => distance,
        };
        here + direction * -distance
    }

    fn outside_edge(&self, edge: EdgeId, co: Vec3) -> Option<VertId> {
        let [v1, v2] = self.mesh.edge_verts(edge);
        let l1 = self.position(v1);
        let (u, length) = normalize_with_length(self.position(v2) - l1);
        let lambda = u.dot(co - l1);
        if lambda <= -EPSILON_BIG * length {
            Some(v1)
        } else if lambda >= (1.0 + EPSILON_BIG) * length {
            Some(v2)
        } else {
            None
        }
    }

    pub(crate) fn angle_kind(&self, e1: HalfId, e2: HalfId, vert: VertId) -> AngleKind {
        let here = self.position(vert);
        let dir1 = normalize(here - self.position(self.other_vert(self.half(e1).edge, vert)));
        let dir2 = normalize(here - self.position(self.other_vert(self.half(e2).edge, vert)));
        if nearly_parallel_normalized(dir1, dir2) {
            return AngleKind::Straight;
        }
        let cross = normalize(dir1.cross(dir2));
        let normal = match (self.half(e1).fnext, self.half(e2).fprev) {
            (Some(face), _) | (None, Some(face)) => self.face_normal(face),
            (None, None) => self.mesh.vert_normal(vert),
        };
        match cross.dot(normal) < 0.0 {
            true => AngleKind::Larger,
            false => AngleKind::Smaller,
        }
    }

    fn point_between_edges(
        &self,
        co: Vec3,
        vert: VertId,
        face: FaceId,
        e1: HalfId,
        e2: HalfId,
    ) -> bool {
        let here = self.position(vert);
        let dir1 = normalize(here - self.position(self.other_vert(self.half(e1).edge, vert)));
        let dir2 = normalize(here - self.position(self.other_vert(self.half(e2).edge, vert)));
        let dirco = normalize(here - co);
        let normal = self.face_normal(face);
        let mut ang11 = angle_normalized(dir1, dir2);
        let mut ang1co = angle_normalized(dir1, dirco);
        if dir1.cross(dir2).dot(normal) < 0.0 {
            ang11 = (std::f64::consts::PI * 2.0) as f32 - ang11;
        }
        if dir1.cross(dirco).dot(normal) < 0.0 {
            ang1co = (std::f64::consts::PI * 2.0) as f32 - ang1co;
        }
        ang11 - ang1co > -EPSILON_ANG
    }

    fn edge_angle_less_than_180(&self, e1: EdgeId, e2: EdgeId, face: FaceId) -> bool {
        let [a1, a2] = self.mesh.edge_verts(e1);
        let [b1, b2] = self.mesh.edge_verts(e2);
        let (vert, v1, v2) = if a1 == b1 {
            (a1, a2, b2)
        } else if a1 == b2 {
            (a1, a2, b1)
        } else if a2 == b1 {
            (a2, a1, b2)
        } else if a2 == b2 {
            (a2, a1, b1)
        } else {
            return false;
        };
        let here = self.position(vert);
        let dir1 = self.position(v1) - here;
        let dir2 = self.position(v2) - here;
        dir1.cross(dir2).dot(self.face_normal(face)) > 0.0
    }

    fn percent_or_absolute_lines(&self, e1: HalfId, e2: HalfId, vert: VertId) -> [Vec3; 4] {
        let v1 = self.other_vert(self.half(e1).edge, vert);
        let v2 = self.other_vert(self.half(e2).edge, vert);
        let fallback = [
            self.position(vert),
            self.position(v1),
            self.position(vert),
            self.position(v2),
        ];
        let (Some(f1), Some(f2)) = (self.half(e1).fnext, self.half(e2).fprev) else {
            return fallback;
        };
        let loop_at = |face: FaceId, at: VertId| self.mesh.face_vert_share_loop(face, at);
        let (Some(l0), Some(l3), Some(l4), Some(l5)) = (
            loop_at(f1, v1),
            loop_at(f2, v2),
            loop_at(f1, vert),
            loop_at(f2, vert),
        ) else {
            return fallback;
        };
        let e0 = self.mesh.loop_edge(l0);
        let e3 = self.mesh.loop_edge(self.mesh.loop_prev(l3));
        let e4 = self.mesh.loop_edge(self.mesh.loop_prev(l4));
        let e5 = self.mesh.loop_edge(l5);
        let (edge1, edge2) = (self.half(e1).edge, self.half(e2).edge);
        let visible = self.edge_angle_less_than_180(e0, edge1, f1)
            && self.edge_angle_less_than_180(edge1, e4, f1)
            && self.edge_angle_less_than_180(edge2, e3, f2)
            && self.edge_angle_less_than_180(e5, edge2, f1);
        if !visible {
            return fallback;
        }
        let offset = self.params.offset;
        let distance = |edge: EdgeId| match self.params.offset_type {
            WidthType::Absolute => offset,
            _ => offset * self.mesh.edge_length(edge) / 100.0,
        };
        let (w1, w2) = match (
            self.params.edge_weight(edge1),
            self.params.edge_weight(edge2),
        ) {
            (Some(w1), Some(w2)) => (w1, w2),
            _ => (1.0, 1.0),
        };
        [
            self.slide_dist(e4, vert, distance(e4) * w1),
            self.slide_dist(e0, v1, distance(e0) * w1),
            self.slide_dist(e5, vert, distance(e5) * w2),
            self.slide_dist(e3, v2, distance(e3) * w2),
        ]
    }

    pub(crate) fn offset_meet(
        &self,
        e1: HalfId,
        e2: HalfId,
        vert: VertId,
        face: Option<FaceId>,
        edges_between: bool,
        in_plane: Option<HalfId>,
    ) -> Vec3 {
        let here = self.position(vert);
        let (h1, h2) = (self.half(e1).clone(), self.half(e2).clone());
        let mut dir1 = here - self.position(self.other_vert(h1.edge, vert));
        let dir2 = self.position(self.other_vert(h2.edge, vert)) - here;
        let (dir1n, dir2p) = match edges_between {
            true => (
                self.position(self.other_vert(self.half(self.next_half(e1)).edge, vert)) - here,
                here - self.position(self.other_vert(self.half(self.prev_half(e2)).edge, vert)),
            ),
            false => (Vec3::ZERO, Vec3::ZERO),
        };
        let mut ang = angle_vectors(dir1, dir2);
        if ang < EPSILON_ANG {
            let norm_v = match face {
                Some(face) => self.face_normal(face),
                None => {
                    let mut count = 0;
                    let mut sum = Vec3::ZERO;
                    let mut current = e1;
                    while current != e2 {
                        if let Some(next_face) = self.half(current).fnext {
                            sum += self.face_normal(next_face);
                            count += 1;
                        }
                        current = self.next_half(current);
                    }
                    match count {
                        0 => self.mesh.vert_normal(vert),
                        _ => sum * (1.0 / count as f32),
                    }
                }
            };
            dir1 += dir2;
            let norm_perp1 = normalize(dir1.cross(norm_v));
            let distance = h1.offset_r.max(h2.offset_l) / (ang / 2.0).cos();
            return here + norm_perp1 * distance;
        }
        if (ang - PI).abs() < EPSILON_ANG {
            let distance = h1.offset_r.max(h2.offset_l);
            return self.slide_dist(h2.edge, vert, distance);
        }
        let reference = |face: Option<FaceId>| match face {
            Some(face) => self.face_normal(face),
            None => self.mesh.vert_normal(vert),
        };
        let (norm_v1, norm_v2) = match (face, edges_between) {
            (Some(face), _) if ang < SMALL_ANG => (self.face_normal(face), self.face_normal(face)),
            (_, false) => {
                let mut normal = normalize(dir2.cross(dir1));
                if normal.dot(reference(face)) < 0.0 {
                    normal = -normal;
                }
                (normal, normal)
            }
            (_, true) => {
                let mut first = normalize(dir1n.cross(dir1));
                if first.dot(reference(h1.fnext)) < 0.0 {
                    first = -first;
                }
                let mut second = normalize(dir2.cross(dir2p));
                if second.dot(reference(h2.fprev)) < 0.0 {
                    second = -second;
                }
                (first, second)
            }
        };
        let norm_perp1 = normalize(dir1.cross(norm_v1));
        let norm_perp2 = normalize(dir2.cross(norm_v2));
        let [off1a, off1b, off2a, off2b] = match self.params.uses_percent_or_absolute() {
            true => self.percent_or_absolute_lines(e1, e2, vert),
            false => {
                let off1a = here + norm_perp1 * h1.offset_r;
                let off2a = here + norm_perp2 * h2.offset_l;
                [off1a, off1a + dir1, off2a, off2a + dir2]
            }
        };
        let meet = line_line(off1a, off1b, off2a, off2b);
        let (mut meetco, second) = match meet {
            LineMeet::Parallel => return off1a,
            LineMeet::Point(point) => (point, None),
            LineMeet::Skew(first, second) => (first, Some(second)),
        };
        if h1.offset_r == 0.0
            && let Some(closer) = self.outside_edge(h1.edge, meetco)
        {
            meetco = self.position(closer);
        }
        if h2.offset_l == 0.0
            && let Some(closer) = self.outside_edge(h2.edge, meetco)
        {
            meetco = self.position(closer);
        }
        if edges_between && h1.offset_r > 0.0 && h2.offset_l > 0.0 {
            if let Some(second) = second {
                meetco = midpoint(meetco, second);
            }
            let mut current = e1;
            while current != e2 {
                let next = self.next_half(current);
                let Some(next_face) = self.half(current).fnext else {
                    current = next;
                    continue;
                };
                let face_normal = self.face_normal(next_face);
                let dropco = Plane::new(here, face_normal).closest_normalized(meetco);
                if let Some(in_plane) = in_plane
                    && let Some(plane_face) = self.half(in_plane).fnext
                {
                    ang = angle_vectors(face_normal, self.face_normal(plane_face));
                    if ang.abs() < SMALL_ANG || (ang - PI).abs() < SMALL_ANG {
                        current = next;
                        continue;
                    }
                }
                if self.point_between_edges(dropco, vert, next_face, current, next) {
                    meetco = dropco;
                    break;
                }
                current = next;
            }
        }
        meetco
    }

    pub(crate) fn offset_meet_edge(&self, e1: HalfId, e2: HalfId, vert: VertId) -> EdgeMeet {
        let here = self.position(vert);
        let dir1 = normalize(self.position(self.other_vert(self.half(e1).edge, vert)) - here);
        let dir2 = normalize(self.position(self.other_vert(self.half(e2).edge, vert)) - here);
        let mut ang = angle_normalized(dir1, dir2);
        if ang.abs() < GOOD_ANGLE {
            return EdgeMeet {
                point: None,
                angle: 0.0,
            };
        }
        if dir1.cross(dir2).dot(self.mesh.vert_normal(vert)) < 0.0 {
            ang = 2.0 * PI - ang;
            return EdgeMeet {
                point: None,
                angle: ang,
            };
        }
        if (ang - PI).abs() < GOOD_ANGLE {
            return EdgeMeet {
                point: None,
                angle: ang,
            };
        }
        let sinang = ang.sin();
        let point = match self.half(e1).offset_r == 0.0 {
            true => here + dir1 * (self.half(e2).offset_l / sinang),
            false => here + dir2 * (self.half(e1).offset_r / sinang),
        };
        EdgeMeet {
            point: Some(point),
            angle: ang,
        }
    }

    pub(crate) fn good_offset_on_edge_between(
        &self,
        e1: HalfId,
        e2: HalfId,
        emid: HalfId,
        vert: VertId,
    ) -> bool {
        self.offset_meet_edge(e1, emid, vert).point.is_some()
            && self.offset_meet_edge(emid, e2, vert).point.is_some()
    }

    pub(crate) fn offset_on_edge_between(
        &self,
        e1: HalfId,
        e2: HalfId,
        emid: HalfId,
        vert: VertId,
    ) -> BetweenMeet {
        let first = self.offset_meet_edge(e1, emid, vert);
        let second = self.offset_meet_edge(emid, e2, vert);
        let ratio = || match first.angle == 0.0 {
            true => 1.0,
            false => second.angle.sin() / first.angle.sin(),
        };
        if self.params.uses_percent_or_absolute() {
            let here = self.position(vert);
            let other = self.position(self.other_vert(self.half(emid).edge, vert));
            let point = match self.params.offset_type {
                WidthType::Percent => {
                    let weight = match (
                        self.params.edge_weight(self.half(e1).edge),
                        self.params.edge_weight(self.half(e2).edge),
                    ) {
                        (Some(w1), Some(w2)) => 0.5 * (w1 + w2),
                        _ => 1.0,
                    };
                    crate::math::lerp(here, other, weight * self.params.offset / 100.0)
                }
                _ => here + normalize(other - here) * self.params.offset,
            };
            return BetweenMeet {
                point,
                sinratio: Some(ratio()),
            };
        }
        match (first.point, second.point) {
            (Some(a), Some(b)) => BetweenMeet {
                point: midpoint(a, b),
                sinratio: Some(ratio()),
            },
            (Some(a), None) => BetweenMeet {
                point: a,
                sinratio: None,
            },
            (None, Some(b)) => BetweenMeet {
                point: b,
                sinratio: None,
            },
            (None, None) => BetweenMeet {
                point: self.slide_dist(self.half(emid).edge, vert, self.half(e1).offset_r),
                sinratio: None,
            },
        }
    }

    pub(crate) fn offset_in_plane(&self, half: HalfId, plane_no: Option<Vec3>, left: bool) -> Vec3 {
        let state = self.half(half);
        let vert = self.half_vert(half);
        let here = self.position(vert);
        let dir = normalize(self.position(self.other_vert(state.edge, vert)) - here);
        let normal = plane_no.unwrap_or_else(|| match dir.x.abs() < dir.y.abs() {
            true => Vec3::X,
            false => Vec3::Y,
        });
        let fdir = match left {
            true => dir.cross(normal),
            false => normal.cross(dir),
        };
        let offset = match left {
            true => state.offset_l,
            false => state.offset_r,
        };
        here + normalize(fdir) * offset
    }

    pub(crate) fn project_to_edge(&self, edge: EdgeId, a: Vec3, b: Vec3) -> Vec3 {
        let [v1, v2] = self.mesh.edge_verts(edge);
        line_line(self.position(v1), self.position(v2), a, b)
            .first()
            .unwrap_or_else(|| self.position(v1))
    }
}
