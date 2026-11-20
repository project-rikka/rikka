use std::f64::consts::PI;

use rikka_physics_math::{Transform, Vec3};

use crate::convex::distance::{DistanceInput, DistanceOutput, SimplexCache, shape_distance};
use crate::convex::support::Support;
use crate::{CastHit, RayInput, ShapeProxy};

const MAX_RAY_ITERATIONS: u32 = 64;

pub(crate) fn rounded_box_moments(half_extents: Vec3, radius: f32) -> (f64, [f64; 3]) {
    let h = half_extents.as_dvec3();
    let r = f64::from(radius);
    let volume = 8.0 * h.x * h.y * h.z
        + 8.0 * r * (h.y * h.z + h.x * h.z + h.x * h.y)
        + 2.0 * PI * r * r * (h.x + h.y + h.z)
        + 4.0 / 3.0 * PI * r.powi(3);
    let mut moments = [0.0; 3];
    for (i, moment) in moments.iter_mut().enumerate() {
        let (hi, hj, hk) = (h[i], h[(i + 1) % 3], h[(i + 2) % 3]);
        let cube = hi.powi(3);
        let quarter_discs =
            PI * r * r * hi * hi + 8.0 / 3.0 * hi * r.powi(3) + PI * r.powi(4) / 4.0;
        *moment = 8.0 / 3.0 * cube * hj * hk
            + 8.0 / 3.0 * ((hi + r).powi(3) - cube) * hj * hk
            + 8.0 / 3.0 * cube * r * (hj + hk)
            + 2.0 / 3.0 * cube * PI * r * r
            + 2.0 * quarter_discs * (hj + hk)
            + 4.0 / 3.0 * PI * r.powi(3) * hi * hi
            + PI * r.powi(4) * hi
            + 4.0 / 15.0 * PI * r.powi(5);
    }
    (volume, moments)
}

fn core_distance<S: Support + ?Sized>(
    core: &S,
    point: Vec3,
    cache: &mut SimplexCache,
) -> DistanceOutput {
    let origin = [Vec3::ZERO];
    let proxy = ShapeProxy::new(&origin, 0.0);
    shape_distance(
        &DistanceInput::new(core, &proxy, Transform::from_translation(point)),
        cache,
    )
}

pub(crate) fn ray_rounded<S: Support + ?Sized>(
    core: &S,
    radius: f32,
    ray: &RayInput,
) -> Option<CastHit> {
    let mut cache = SimplexCache::EMPTY;
    let mut output = core_distance(core, ray.origin, &mut cache);
    if output.distance <= radius {
        return Some(CastHit::new(ray.origin, Vec3::ZERO, 0.0));
    }
    let length = ray.translation.length();
    if length == 0.0 {
        return None;
    }
    let direction = ray.translation / length;
    let max_distance = ray.max_fraction * length;
    let tolerance = 1.0e-5 * (1.0 + radius);
    let mut t = 0.0f32;
    for _ in 0..MAX_RAY_ITERATIONS {
        let gap = output.distance - radius;
        let normal = (output.point_b - output.point_a) / output.distance;
        if gap <= tolerance {
            let fraction = t / length;
            return Some(CastHit::new(
                ray.origin + fraction * ray.translation,
                normal,
                fraction,
            ));
        }
        let approach = -normal.dot(direction);
        if approach <= 0.0 {
            return None;
        }
        t += gap / approach;
        if t > max_distance {
            return None;
        }
        output = core_distance(core, ray.origin + t * direction, &mut cache);
    }
    None
}
