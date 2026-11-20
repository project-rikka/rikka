use rikka_physics_math::{Transform, Vec3};

use super::cache::SimplexCache;
use super::epa::penetration;
use super::gjk::{DistanceInput, gjk};
use crate::convex::support::Support;

const SHALLOW: f32 = 100.0 * f32::EPSILON;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ClosestFeatures {
    pub(crate) normal: Vec3,
    pub(crate) separation: f32,
    pub(crate) point_a: Vec3,
    pub(crate) point_b: Vec3,
}

pub(crate) fn closest_features<A: Support + ?Sized, B: Support + ?Sized>(
    a: &A,
    b: &B,
    transform: Transform,
    cache: &mut SimplexCache,
) -> Option<ClosestFeatures> {
    let radius_a = a.radius();
    let radius_b = b.radius();
    let (output, simplex) = gjk(&DistanceInput::new(a, b, transform), cache);
    if output.distance > SHALLOW {
        return Some(ClosestFeatures {
            normal: output.normal,
            separation: output.distance - radius_a - radius_b,
            point_a: output.point_a + radius_a * output.normal,
            point_b: output.point_b - radius_b * output.normal,
        });
    }
    cache.reset();
    let deep = penetration(a, b, transform, &simplex)?;
    Some(ClosestFeatures {
        normal: deep.normal,
        separation: -deep.depth - radius_a - radius_b,
        point_a: deep.point_a + radius_a * deep.normal,
        point_b: deep.point_b - radius_b * deep.normal,
    })
}
