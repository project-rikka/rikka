use rikka_physics_math::{Vec3W, Wide};

use super::{BatchManifold, CapsuleLanes, TransformLanes, linear_slop};

const DEGENERATE: f32 = 100.0 * f32::EPSILON;
const DETERMINANT: f32 = 1000.0 * f32::MIN_POSITIVE;
const PARALLEL: f32 = 1.01 * 0.05 * 0.05;
const SHORT: f32 = 0.99;

#[derive(Clone, Copy)]
struct Closest<W: Wide> {
    point1: Vec3W<W>,
    point2: Vec3W<W>,
}

#[inline(always)]
fn segment_distance<W: Wide>(p1: Vec3W<W>, q1: Vec3W<W>, p2: Vec3W<W>, q2: Vec3W<W>) -> Closest<W> {
    let zero = W::default();
    let one = W::one();
    let d1 = q1 - p1;
    let d2 = q2 - p2;
    let r = p1 - p2;
    let a = d1.dot(d1);
    let b = d1.dot(d2);
    let c = d1.dot(r);
    let e = d2.dot(d2);
    let f = d2.dot(r);
    let degenerate = W::splat(DEGENERATE);
    let point_a = a.simd_lt(degenerate);
    let point_b = e.simd_lt(degenerate);

    let denominator = a * e - b * b;
    let s1 = W::select(
        denominator.simd_gt(W::splat(DETERMINANT)),
        ((b * f - c * e) / denominator).clamp(zero, one),
        zero,
    );
    let s2 = (b * s1 + f) / e;
    let below = s2.simd_lt(zero);
    let above = s2.simd_gt(one);
    let s1 = W::select(
        below,
        (-c / a).clamp(zero, one),
        W::select(above, ((b - c) / a).clamp(zero, one), s1),
    );
    let s2 = s2.clamp(zero, one);

    let s1 = W::select(
        point_a,
        zero,
        W::select(point_b, (-c / a).clamp(zero, one), s1),
    );
    let s2 = W::select(
        point_b,
        zero,
        W::select(point_a, (f / e).clamp(zero, one), s2),
    );
    Closest {
        point1: d1.mul_add(s1, p1),
        point2: d2.mul_add(s2, p2),
    }
}

#[inline(always)]
pub fn collide_batch_capsules<W: Wide>(
    a: &CapsuleLanes<W>,
    b: &CapsuleLanes<W>,
    transform: &TransformLanes<W>,
    speculative: W,
) -> BatchManifold<W> {
    let a1 = a.center1;
    let a2 = a.center2;
    let b1 = transform.apply(b.center1);
    let b2 = transform.apply(b.center2);
    let radius = a.radius1 + b.radius1;
    let max_distance = radius + speculative;
    let slop = linear_slop(speculative);
    let min_distance = W::splat(0.01) * slop;

    let closest = segment_distance(a1, a2, b1, b2);
    let offset = closest.point2 - closest.point1;
    let distance_squared = offset.length_squared();
    let touching = distance_squared.simd_le(max_distance * max_distance)
        & distance_squared.simd_ge(min_distance * min_distance);

    let segment_a = a2 - a1;
    let segment_b = b2 - b1;
    let length_a = segment_a.length();
    let length_b = segment_b.length();
    let cross = segment_a.cross(segment_b).length_squared();
    let short = W::splat(SHORT) * slop;
    let parallel = length_a.simd_ge(short)
        & length_b.simd_ge(short)
        & cross.simd_lt(W::splat(PARALLEL) * (length_a * length_a) * (length_b * length_b));

    let distance = distance_squared.sqrt();
    let normal = offset.scale(W::one() / distance);
    let point = (normal.mul_add(a.radius1, closest.point1)
        + normal.mul_add(-b.radius1, closest.point2))
    .scale(W::splat(0.5));
    BatchManifold {
        normal,
        point,
        separation: distance - radius,
        touching,
        fallback: a.tapered() | b.tapered() | (parallel & touching),
    }
}
