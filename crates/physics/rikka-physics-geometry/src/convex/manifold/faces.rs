use std::f32::consts::TAU;

use rikka_physics_math::{Transform, Vec3, cos_sin};

use crate::convex::Convex;
use crate::convex::capsule::Solid;
use crate::convex::hull::Polyhedron;

pub(crate) const MAX_FACE_POINTS: usize = 64;
const CAP_SIDES: u16 = 16;
const SIDE_ID: u16 = 2 * CAP_SIDES;
const RIM_ID: u16 = SIDE_ID + 2;
pub(crate) const FLAT: f32 = 0.998;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Face {
    points: [Vec3; MAX_FACE_POINTS],
    ids: [u16; MAX_FACE_POINTS],
    count: usize,
}

impl Default for Face {
    fn default() -> Self {
        Self {
            points: [Vec3::ZERO; MAX_FACE_POINTS],
            ids: [0; MAX_FACE_POINTS],
            count: 0,
        }
    }
}

impl Face {
    fn push(&mut self, point: Vec3, id: u16) {
        if self.count < MAX_FACE_POINTS {
            self.points[self.count] = point;
            self.ids[self.count] = id;
            self.count += 1;
        }
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.count
    }

    #[inline]
    pub(crate) fn points(&self) -> &[Vec3] {
        &self.points[..self.count]
    }

    #[inline]
    pub(crate) fn ids(&self) -> &[u16] {
        &self.ids[..self.count]
    }

    pub(crate) fn transformed(mut self, transform: Transform) -> Self {
        for point in &mut self.points[..self.count] {
            *point = transform.apply(*point);
        }
        self
    }

    fn polyhedron(polyhedron: &Polyhedron<'_>, direction: Vec3, radius: f32) -> Self {
        let mut face = Self::default();
        let index = polyhedron.support_face(direction);
        let offset = radius * polyhedron.planes[index].normal;
        for edge in polyhedron.face_edges(index) {
            let origin = polyhedron.edges[edge].origin;
            face.push(
                polyhedron.points[origin as usize] + offset,
                u16::from(origin),
            );
        }
        face
    }
}

pub(crate) fn supporting_face(shape: Convex<'_>, direction: Vec3) -> Face {
    let direction = direction.normalize_or_zero();
    let mut face = Face::default();
    match shape {
        Convex::Sphere(sphere) => face.push(sphere.center + sphere.radius * direction, 0),
        Convex::Capsule(capsule) => match capsule.solid() {
            Solid::Ball(sphere) => face.push(sphere.center + sphere.radius * direction, 0),
            Solid::Swept {
                axis, sine, cosine, ..
            } => {
                let radial = (direction - direction.dot(axis) * axis).normalize_or_zero();
                let side = sine * axis + cosine * radial;
                if radial != Vec3::ZERO && side.dot(direction) > FLAT {
                    face.push(capsule.center1 + capsule.radius1 * side, 0);
                    face.push(capsule.center2 + capsule.radius2 * side, 1);
                } else if capsule.center1.dot(direction) + capsule.radius1
                    >= capsule.center2.dot(direction) + capsule.radius2
                {
                    face.push(capsule.center1 + capsule.radius1 * direction, 0);
                } else {
                    face.push(capsule.center2 + capsule.radius2 * direction, 1);
                }
            }
        },
        Convex::Cuboid(cuboid) => {
            return Face::polyhedron(&cuboid.polyhedron().view(), direction, cuboid.radius);
        }
        Convex::Hull(hull) => {
            return Face::polyhedron(&hull.polyhedron(), direction, hull.radius());
        }
        Convex::Cylinder(cylinder) => {
            let local = cylinder.transform.rotate_inverse(direction);
            if local.y.abs() > FLAT {
                let sign = local.y.signum();
                let base = if sign > 0.0 { 0 } else { CAP_SIDES };
                for index in 0..CAP_SIDES {
                    let angle = cos_sin(TAU * f32::from(index) / f32::from(CAP_SIDES));
                    let point = Vec3::new(
                        cylinder.radius * angle.cosine,
                        sign * cylinder.half_height,
                        -sign * cylinder.radius * angle.sine,
                    );
                    face.push(cylinder.transform.apply(point), base + index);
                }
            } else if local.y.abs() < (1.0 - FLAT * FLAT).sqrt() {
                let rim = Vec3::new(local.x, 0.0, local.z).normalize_or_zero() * cylinder.radius;
                let low = rim - Vec3::new(0.0, cylinder.half_height, 0.0);
                let high = rim + Vec3::new(0.0, cylinder.half_height, 0.0);
                face.push(cylinder.transform.apply(low), SIDE_ID);
                face.push(cylinder.transform.apply(high), SIDE_ID + 1);
            } else {
                face.push(
                    cylinder.transform.apply(cylinder.local_support(local)),
                    RIM_ID,
                );
            }
        }
    }
    face
}
