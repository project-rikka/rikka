use rikka_physics_math::{Mat3, Transform, Vec3, perpendicular};

use super::simplex::Simplex;
use crate::convex::support::Support;

const MAX_VERTICES: usize = 128;
const MAX_FACES: usize = 256;
const MAX_HORIZON: usize = 128;
const RELATIVE_TOLERANCE: f32 = 1.0e-4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Penetration {
    pub(crate) normal: Vec3,
    pub(crate) depth: f32,
    pub(crate) point_a: Vec3,
    pub(crate) point_b: Vec3,
}

#[derive(Clone, Copy, Default)]
struct Vertex {
    a: Vec3,
    b: Vec3,
    w: Vec3,
}

#[derive(Clone, Copy, Default)]
struct Face {
    indices: [u8; 3],
    normal: Vec3,
    distance: f32,
}

struct Polytope<'s, A: ?Sized, B: ?Sized> {
    a: &'s A,
    b: &'s B,
    rotation: Mat3,
    inverse: Mat3,
    translation: Vec3,
    vertices: [Vertex; MAX_VERTICES],
    vertex_count: usize,
    faces: [Face; MAX_FACES],
    face_count: usize,
}

impl<'s, A: Support + ?Sized, B: Support + ?Sized> Polytope<'s, A, B> {
    fn support(&self, direction: Vec3) -> Vertex {
        let a = self.a.support(-direction).point;
        let b = self.rotation * self.b.support(self.inverse * direction).point + self.translation;
        Vertex { a, b, w: b - a }
    }

    fn push_vertex(&mut self, vertex: Vertex) -> Option<u8> {
        if self.vertex_count == MAX_VERTICES {
            return None;
        }
        self.vertices[self.vertex_count] = vertex;
        self.vertex_count += 1;
        Some((self.vertex_count - 1) as u8)
    }

    fn push_face(&mut self, i: u8, j: u8, k: u8) -> bool {
        if self.face_count == MAX_FACES {
            return false;
        }
        let [a, b, c] = [i, j, k].map(|index| self.vertices[index as usize].w);
        let normal = (b - a).cross(c - a);
        let length = normal.length();
        let (normal, distance) = if length > 1.0e-12 {
            let normal = normal / length;
            (normal, normal.dot(a))
        } else {
            (Vec3::ZERO, f32::MAX)
        };
        self.faces[self.face_count] = Face {
            indices: [i, j, k],
            normal,
            distance,
        };
        self.face_count += 1;
        true
    }

    fn closest_face(&self) -> Option<usize> {
        let mut best = None;
        let mut best_distance = f32::MAX;
        for (index, face) in self.faces[..self.face_count].iter().enumerate() {
            if face.distance < best_distance {
                best_distance = face.distance;
                best = Some(index);
            }
        }
        best
    }

    fn seed(&mut self, simplex: &Simplex) -> bool {
        for vertex in &simplex.vertices[..simplex.count] {
            let w = vertex.w;
            self.push_vertex(Vertex {
                a: vertex.point_a,
                b: vertex.point_b(),
                w,
            });
        }
        let scale = self.scale().max(f32::EPSILON);
        let epsilon = 1.0e-5 * scale;

        if self.vertex_count == 1 {
            let axes = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z];
            let origin = self.vertices[0].w;
            let Some(vertex) = axes
                .iter()
                .map(|&axis| self.support(axis))
                .find(|vertex| vertex.w.distance(origin) > epsilon)
            else {
                return false;
            };
            self.push_vertex(vertex);
        }

        if self.vertex_count == 2 {
            let p = self.vertices[0].w;
            let axis = (self.vertices[1].w - p).normalize_or_zero();
            let side = perpendicular(axis);
            let rotation = Mat3::from_axis_angle(axis, std::f32::consts::FRAC_PI_3);
            let mut direction = side;
            let mut found = None;
            for _ in 0..6 {
                let vertex = self.support(direction);
                let offset = vertex.w - p;
                if (offset - offset.dot(axis) * axis).length() > epsilon {
                    found = Some(vertex);
                    break;
                }
                direction = rotation * direction;
            }
            let Some(vertex) = found else {
                return false;
            };
            self.push_vertex(vertex);
        }

        if self.vertex_count == 3 {
            let [p, q, r] = [0, 1, 2].map(|index| self.vertices[index].w);
            let normal = (q - p).cross(r - p).normalize_or_zero();
            if normal == Vec3::ZERO {
                return false;
            }
            let above = self.support(normal);
            let below = self.support(-normal);
            let height_above = (above.w - p).dot(normal);
            let height_below = (p - below.w).dot(normal);
            let vertex = if height_above >= height_below {
                above
            } else {
                below
            };
            if height_above.max(height_below) <= epsilon {
                return false;
            }
            self.push_vertex(vertex);
        }

        let [p, q, r, s] = [0, 1, 2, 3].map(|index| self.vertices[index].w);
        if (q - p).cross(r - p).dot(s - p).abs() <= epsilon * epsilon * epsilon {
            return false;
        }
        let orient = |this: &mut Self, i: u8, j: u8, k: u8, other: Vec3| {
            let [a, b, c] = [i, j, k].map(|index| this.vertices[index as usize].w);
            if (b - a).cross(c - a).dot(other - a) > 0.0 {
                this.push_face(i, k, j)
            } else {
                this.push_face(i, j, k)
            }
        };
        orient(self, 0, 1, 2, s);
        orient(self, 0, 1, 3, r);
        orient(self, 0, 2, 3, q);
        orient(self, 1, 2, 3, p);

        self.faces[..self.face_count]
            .iter()
            .all(|face| face.distance >= -epsilon)
    }

    fn scale(&self) -> f32 {
        self.vertices[..self.vertex_count]
            .iter()
            .map(|vertex| vertex.w.length())
            .fold(0.0, f32::max)
    }

    fn expand(&mut self, face: usize, vertex: Vertex) -> bool {
        if self.vertex_count == MAX_VERTICES {
            return false;
        }
        let point = vertex.w;
        let mut visible = [false; MAX_FACES];
        let mut visible_count = 0;
        let mut horizon = [[0u8; 2]; MAX_HORIZON];
        let mut horizon_count = 0;

        for (index, (candidate, visible)) in self.faces[..self.face_count]
            .iter()
            .zip(visible.iter_mut())
            .enumerate()
        {
            let base = self.vertices[candidate.indices[0] as usize].w;
            if index != face && candidate.normal.dot(point - base) <= 0.0 {
                continue;
            }
            *visible = true;
            visible_count += 1;
            for edge in 0..3 {
                let from = candidate.indices[edge];
                let to = candidate.indices[(edge + 1) % 3];
                if let Some(shared) = horizon[..horizon_count]
                    .iter()
                    .position(|&[x, y]| x == to && y == from)
                {
                    horizon[shared] = horizon[horizon_count - 1];
                    horizon_count -= 1;
                } else {
                    if horizon_count == MAX_HORIZON {
                        return false;
                    }
                    horizon[horizon_count] = [from, to];
                    horizon_count += 1;
                }
            }
        }

        if self.face_count - visible_count + horizon_count > MAX_FACES {
            return false;
        }

        let mut live = 0;
        for index in (0..self.face_count).filter(|&index| !visible[index]) {
            self.faces[live] = self.faces[index];
            live += 1;
        }
        self.face_count = live;
        let Some(apex) = self.push_vertex(vertex) else {
            return false;
        };
        horizon[..horizon_count]
            .iter()
            .all(|&[from, to]| self.push_face(from, to, apex))
    }

    fn result(&self, face: usize) -> Penetration {
        let face = self.faces[face];
        let [a, b, c] = face.indices.map(|index| self.vertices[index as usize]);
        let projection = face.distance * face.normal;
        let weights = barycentric(a.w, b.w, c.w, projection);
        Penetration {
            normal: -face.normal,
            depth: face.distance.max(0.0),
            point_a: weights[0] * a.a + weights[1] * b.a + weights[2] * c.a,
            point_b: weights[0] * a.b + weights[1] * b.b + weights[2] * c.b,
        }
    }
}

fn barycentric(a: Vec3, b: Vec3, c: Vec3, p: Vec3) -> [f32; 3] {
    let v0 = b - a;
    let v1 = c - a;
    let v2 = p - a;
    let d00 = v0.dot(v0);
    let d01 = v0.dot(v1);
    let d11 = v1.dot(v1);
    let d20 = v2.dot(v0);
    let d21 = v2.dot(v1);
    let denominator = d00 * d11 - d01 * d01;
    if denominator.abs() <= f32::MIN_POSITIVE {
        return [1.0, 0.0, 0.0];
    }
    let v = (d11 * d20 - d01 * d21) / denominator;
    let w = (d00 * d21 - d01 * d20) / denominator;
    [1.0 - v - w, v, w]
}

pub(crate) fn penetration<A: Support + ?Sized, B: Support + ?Sized>(
    a: &A,
    b: &B,
    transform: Transform,
    simplex: &Simplex,
) -> Option<Penetration> {
    let rotation = Mat3::from_quat(transform.q);
    let mut polytope = Polytope {
        a,
        b,
        rotation,
        inverse: rotation.transpose(),
        translation: transform.p,
        vertices: [Vertex::default(); MAX_VERTICES],
        vertex_count: 0,
        faces: [Face::default(); MAX_FACES],
        face_count: 0,
    };
    if simplex.count == 0 || !polytope.seed(simplex) {
        return None;
    }
    let tolerance = RELATIVE_TOLERANCE * polytope.scale().max(f32::EPSILON);

    loop {
        let face = polytope.closest_face()?;
        let closest = polytope.faces[face];
        let vertex = polytope.support(closest.normal);
        let gap = vertex.w.dot(closest.normal) - closest.distance;
        if gap <= tolerance {
            return Some(polytope.result(face));
        }
        if !polytope.expand(face, vertex) {
            return Some(polytope.result(face));
        }
    }
}
