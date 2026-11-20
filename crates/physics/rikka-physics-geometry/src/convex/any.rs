use rikka_physics_math::Vec3;

use crate::convex::capsule::Capsule;
use crate::convex::cuboid::Cuboid;
use crate::convex::cylinder::Cylinder;
use crate::convex::hull::ConvexHull;
use crate::convex::queries::{PointProjection, project_point};
use crate::convex::sphere::Sphere;
use crate::convex::support::{Support, SupportPoint};

#[derive(Clone, Copy, Debug)]
pub enum Convex<'a> {
    Sphere(&'a Sphere),
    Capsule(&'a Capsule),
    Cuboid(&'a Cuboid),
    Cylinder(&'a Cylinder),
    Hull(&'a ConvexHull),
}

impl Convex<'_> {
    pub fn project_point(&self, point: Vec3) -> Option<PointProjection> {
        match self {
            Self::Sphere(sphere) => {
                let surface = crate::convex::capsule::ball_closest(
                    sphere.center,
                    sphere.radius,
                    point,
                    Vec3::Y,
                );
                Some(PointProjection {
                    point: surface.point,
                    normal: surface.normal,
                    distance: surface.distance,
                })
            }
            Self::Capsule(capsule) => {
                let surface = capsule.closest(point);
                Some(PointProjection {
                    point: surface.point,
                    normal: surface.normal,
                    distance: surface.distance,
                })
            }
            _ => project_point(self, point),
        }
    }
}

impl Support for Convex<'_> {
    #[inline]
    fn support(&self, direction: Vec3) -> SupportPoint {
        match self {
            Self::Sphere(shape) => shape.support(direction),
            Self::Capsule(shape) => shape.support(direction),
            Self::Cuboid(shape) => shape.support(direction),
            Self::Cylinder(shape) => shape.support(direction),
            Self::Hull(shape) => shape.support(direction),
        }
    }

    #[inline]
    fn radius(&self) -> f32 {
        match self {
            Self::Sphere(shape) => shape.radius(),
            Self::Capsule(shape) => shape.radius(),
            Self::Cuboid(shape) => shape.radius(),
            Self::Cylinder(shape) => shape.radius(),
            Self::Hull(shape) => shape.radius(),
        }
    }

    #[inline]
    fn anchor(&self) -> SupportPoint {
        match self {
            Self::Sphere(shape) => shape.anchor(),
            Self::Capsule(shape) => shape.anchor(),
            Self::Cuboid(shape) => shape.anchor(),
            Self::Cylinder(shape) => shape.anchor(),
            Self::Hull(shape) => shape.anchor(),
        }
    }

    #[inline]
    fn vertex(&self, id: u32) -> Option<Vec3> {
        match self {
            Self::Sphere(shape) => shape.vertex(id),
            Self::Capsule(shape) => shape.vertex(id),
            Self::Cuboid(shape) => shape.vertex(id),
            Self::Cylinder(shape) => shape.vertex(id),
            Self::Hull(shape) => shape.vertex(id),
        }
    }
}
