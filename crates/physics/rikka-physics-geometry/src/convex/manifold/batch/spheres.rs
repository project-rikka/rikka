use rikka_physics_math::{Mask, Vec3, Vec3W, Wide};

use super::{BatchManifold, CapsuleLanes, SphereLanes, TransformLanes};

const DEGENERATE_LENGTH: f32 = 1000.0 * f32::MIN_POSITIVE;
const DEGENERATE_AXIS: f32 = 1.0e-6;
const DEGENERATE_RADIAL: f32 = 1.0e-9;

#[inline(always)]
fn ball_contact<W: Wide>(
    center: Vec3W<W>,
    radius: W,
    other: Vec3W<W>,
    other_radius: W,
    speculative: W,
) -> BatchManifold<W> {
    let offset = other - center;
    let length = offset.length();
    let separated = (length * length).simd_gt(W::splat(DEGENERATE_LENGTH));
    let normal = Vec3W::select(
        separated,
        offset.scale(W::one() / length),
        Vec3W::splat(Vec3::Y),
    );
    let separation = length - radius - other_radius;
    let surface = normal.mul_add(radius, center);
    BatchManifold {
        normal,
        point: (surface + normal.mul_add(-other_radius, other)).scale(W::splat(0.5)),
        separation,
        touching: separation.simd_le(speculative),
        fallback: W::Mask::splat(false),
    }
}

#[inline(always)]
pub fn collide_batch_spheres<W: Wide>(
    a: &SphereLanes<W>,
    b: &SphereLanes<W>,
    transform: &TransformLanes<W>,
    speculative: W,
) -> BatchManifold<W> {
    ball_contact(
        a.center,
        a.radius,
        transform.apply(b.center),
        b.radius,
        speculative,
    )
}

#[inline(always)]
pub fn collide_batch_capsule_sphere<W: Wide>(
    a: &CapsuleLanes<W>,
    b: &SphereLanes<W>,
    transform: &TransformLanes<W>,
    speculative: W,
) -> BatchManifold<W> {
    let center = transform.apply(b.center);
    let delta = a.center2 - a.center1;
    let length = delta.length();
    let axis = delta.scale(W::one() / length);
    let offset = center - a.center1;
    let along = offset.dot(axis);
    let radial = (offset - axis.scale(along)).length();
    let clamped = along.clamp(W::default(), length);
    let closest = axis.mul_add(clamped, a.center1);
    let mut contact = ball_contact(closest, a.radius1, center, b.radius, speculative);
    let degenerate = length.simd_le(W::splat(DEGENERATE_AXIS) * a.radius1)
        | radial.simd_le(W::splat(DEGENERATE_RADIAL) * length)
        | (center - closest)
            .length_squared()
            .simd_le(W::splat(DEGENERATE_LENGTH));
    contact.fallback = a.tapered() | degenerate;
    contact
}
