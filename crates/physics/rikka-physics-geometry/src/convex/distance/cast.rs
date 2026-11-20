use rikka_physics_math::{Transform, Vec3};

use super::cache::SimplexCache;
use super::gjk::{DistanceInput, is_normalized, shape_distance};
use crate::convex::support::Support;
use crate::{CastHit, ShapeProxy, Tolerance};

const MAX_ITERATIONS: u32 = 20;

#[derive(Debug)]
pub struct ShapeCastPairInput<'a, A: ?Sized = ShapeProxy<'a>, B: ?Sized = ShapeProxy<'a>> {
    pub a: &'a A,
    pub b: &'a B,
    pub transform: Transform,
    pub translation_b: Vec3,
    pub max_fraction: f32,
    pub can_encroach: bool,
    pub tolerance: Tolerance,
}

impl<'a, A: Support + ?Sized, B: Support + ?Sized> ShapeCastPairInput<'a, A, B> {
    #[inline]
    pub fn new(a: &'a A, b: &'a B, transform: Transform, translation_b: Vec3) -> Self {
        Self {
            a,
            b,
            transform,
            translation_b,
            max_fraction: 1.0,
            can_encroach: false,
            tolerance: Tolerance::default(),
        }
    }
}

pub fn shape_cast_pair<A: Support + ?Sized, B: Support + ?Sized>(
    input: &ShapeCastPairInput<'_, A, B>,
) -> Option<CastHit> {
    let linear_slop = input.tolerance.linear_slop();
    let radius_a = input.a.radius();
    let radius_b = input.b.radius();
    let mut target = linear_slop.max(radius_a + radius_b - linear_slop);
    let tolerance = 0.25 * linear_slop;

    let mut cache = SimplexCache::EMPTY;
    let mut distance_input = DistanceInput::new(input.a, input.b, input.transform);
    let delta = input.translation_b;
    let mut alpha = 0.0f32;

    for iteration in 0..MAX_ITERATIONS {
        let output = shape_distance(&distance_input, &mut cache);

        if output.distance < target + tolerance {
            if iteration == 0 {
                if input.can_encroach && output.distance > 2.0 * linear_slop {
                    target = output.distance - linear_slop;
                } else {
                    let c1 = output.point_a + radius_a * output.normal;
                    let c2 = output.point_b - radius_b * output.normal;
                    return Some(CastHit::new(c1.lerp(c2, 0.5), Vec3::ZERO, 0.0));
                }
            } else {
                if output.distance > 0.0 && !is_normalized(output.normal) {
                    return None;
                }
                return Some(CastHit::new(
                    output.point_a + radius_a * output.normal,
                    output.normal,
                    alpha,
                ));
            }
        }

        let denominator = delta.dot(output.normal);
        if denominator >= 0.0 {
            return None;
        }

        alpha += (target - output.distance) / denominator;
        if alpha >= input.max_fraction {
            return None;
        }

        distance_input.transform.p = input.transform.p + alpha * delta;
    }

    None
}
