use rikka_physics_math::{Aabb, Plane, Transform, Vec3};

use crate::convex::queries::overlap_slop;
use crate::{CastHit, MassData, RayInput, ShapeCastInput, ShapeGeometry, ShapeProxy};

pub const PLANE_EXTENT: f32 = 1.0e6;

impl ShapeGeometry for Plane {
    fn aabb(&self, transform: Transform) -> Aabb {
        let normal = transform.rotate(self.normal);
        let offset = self.offset + normal.dot(transform.p);
        let mut min = Vec3::splat(-PLANE_EXTENT);
        let mut max = Vec3::splat(PLANE_EXTENT);
        for axis in 0..3 {
            let component = normal[axis];
            if (component.abs() - 1.0).abs() <= 1.0e-6 {
                if component > 0.0 {
                    max[axis] = offset;
                } else {
                    min[axis] = -offset;
                }
            }
        }
        Aabb::new(min, max)
    }

    fn mass(&self, _density: f32) -> MassData {
        MassData::ZERO
    }

    fn ray_cast(&self, ray: &RayInput) -> Option<CastHit> {
        let start = self.separation(ray.origin);
        if start <= 0.0 {
            return Some(CastHit::new(ray.origin, Vec3::ZERO, 0.0));
        }
        let approach = self.normal.dot(ray.translation);
        if approach >= 0.0 {
            return None;
        }
        let fraction = -start / approach;
        (fraction <= ray.max_fraction).then(|| {
            CastHit::new(
                ray.origin + fraction * ray.translation,
                self.normal,
                fraction,
            )
        })
    }

    fn shape_cast(&self, cast: &ShapeCastInput<'_>) -> Option<CastHit> {
        let proxy = &cast.proxy;
        let (deepest, start) = proxy
            .points
            .iter()
            .map(|&point| (point, self.separation(point) - proxy.radius))
            .min_by(|a, b| a.1.total_cmp(&b.1))?;
        let contact = deepest - proxy.radius * self.normal;
        if start <= 0.0 {
            return Some(CastHit::new(contact - start * self.normal, Vec3::ZERO, 0.0));
        }
        let approach = self.normal.dot(cast.translation);
        if approach >= 0.0 {
            return None;
        }
        let fraction = -start / approach;
        (fraction <= cast.max_fraction)
            .then(|| CastHit::new(contact + fraction * cast.translation, self.normal, fraction))
    }

    fn overlaps(&self, transform: Transform, proxy: &ShapeProxy<'_>) -> bool {
        let normal = transform.rotate(self.normal);
        let offset = self.offset + normal.dot(transform.p);
        proxy
            .points
            .iter()
            .any(|&point| normal.dot(point) - offset - proxy.radius < overlap_slop())
    }
}

pub(crate) fn validate_plane(plane: &Plane) -> crate::Result<()> {
    crate::convex::queries::check_point("plane normal", plane.normal)?;
    if !plane.offset.is_finite() {
        return Err(crate::Error::NonFinite {
            what: "plane offset",
        });
    }
    let length = plane.normal.length();
    if (length - 1.0).abs() > 1.0e-3 {
        return Err(crate::Error::NotUnitVector {
            what: "plane normal",
            length,
        });
    }
    Ok(())
}
