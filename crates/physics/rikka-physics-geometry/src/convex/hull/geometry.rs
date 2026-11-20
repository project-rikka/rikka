use rikka_physics_math::{Aabb, Transform, Vec3};

use super::blob::ConvexHull;
use super::rounded::rounded_mass;
use super::view::Polyhedron;
use crate::convex::queries;
use crate::convex::rounding::ray_rounded;
use crate::convex::support::{Support, SupportPoint};
use crate::{CastHit, MassData, RayInput, ShapeCastInput, ShapeGeometry, ShapeProxy};

impl Support for Polyhedron<'_> {
    #[inline]
    fn support(&self, direction: Vec3) -> SupportPoint {
        let index = self.support_vertex(direction);
        SupportPoint::new(self.points[index], index as u32)
    }

    #[inline]
    fn radius(&self) -> f32 {
        0.0
    }

    #[inline]
    fn anchor(&self) -> SupportPoint {
        SupportPoint::new(self.points[0], 0)
    }

    #[inline]
    fn vertex(&self, id: u32) -> Option<Vec3> {
        self.points.get(id as usize).copied()
    }
}

impl Support for ConvexHull {
    #[inline]
    fn support(&self, direction: Vec3) -> SupportPoint {
        self.polyhedron().support(direction)
    }

    #[inline]
    fn radius(&self) -> f32 {
        self.radius()
    }

    #[inline]
    fn anchor(&self) -> SupportPoint {
        SupportPoint::new(self.points()[0], 0)
    }

    #[inline]
    fn vertex(&self, id: u32) -> Option<Vec3> {
        self.points().get(id as usize).copied()
    }
}

fn ray_polyhedron(polyhedron: &Polyhedron<'_>, ray: &RayInput) -> Option<CastHit> {
    let mut lower = 0.0f32;
    let mut upper = ray.max_fraction;
    let mut best_face = None;
    for (index, plane) in polyhedron.planes.iter().enumerate() {
        let distance = plane.offset - plane.normal.dot(ray.origin);
        let denominator = plane.normal.dot(ray.translation);
        if denominator == 0.0 {
            if distance < 0.0 {
                return None;
            }
            continue;
        }
        let fraction = distance / denominator;
        if denominator < 0.0 {
            if fraction > lower {
                best_face = Some(index);
                lower = fraction;
            }
        } else if fraction < upper {
            upper = fraction;
        }
        if upper < lower {
            return None;
        }
    }
    Some(match best_face {
        Some(face) => CastHit::new(
            ray.origin + lower * ray.translation,
            polyhedron.planes[face].normal,
            lower,
        ),
        None => CastHit::new(ray.origin, Vec3::ZERO, 0.0),
    })
}

impl ShapeGeometry for ConvexHull {
    fn aabb(&self, transform: Transform) -> Aabb {
        self.bounds().transformed(transform).inflated(self.radius())
    }

    fn mass(&self, density: f32) -> MassData {
        let header = self.header();
        let core = MassData {
            mass: header.volume,
            center: header.center,
            inertia: header.unit_inertia,
        };
        if self.radius() > 0.0 {
            return rounded_mass(&self.polyhedron(), core, self.radius(), density);
        }
        MassData {
            mass: density * core.mass,
            center: core.center,
            inertia: core.inertia * density,
        }
    }

    fn ray_cast(&self, ray: &RayInput) -> Option<CastHit> {
        if self.radius() > 0.0 {
            return ray_rounded(&self.polyhedron(), self.radius(), ray);
        }
        ray_polyhedron(&self.polyhedron(), ray)
    }

    fn shape_cast(&self, cast: &ShapeCastInput<'_>) -> Option<CastHit> {
        queries::shape_cast(self, cast)
    }

    fn overlaps(&self, transform: Transform, proxy: &ShapeProxy<'_>) -> bool {
        queries::overlaps(self, transform, proxy)
    }
}
