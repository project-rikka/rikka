use rikka_physics_math::{Vec3W, Wide};

use super::{BatchManifold, CuboidLanes, SphereLanes, TransformLanes};

const SHALLOW: f32 = 1000.0 * f32::EPSILON;

#[inline(always)]
pub fn collide_batch_cuboid_sphere<W: Wide>(
    a: &CuboidLanes<W>,
    b: &SphereLanes<W>,
    transform: &TransformLanes<W>,
    speculative: W,
) -> BatchManifold<W> {
    let center = transform.apply(b.center);
    let local = a.transform.apply_inverse(center);
    let extent = a.half_extents;
    let clamped = Vec3W::new(
        local.x.clamp(-extent.x, extent.x),
        local.y.clamp(-extent.y, extent.y),
        local.z.clamp(-extent.z, extent.z),
    );
    let surface = a.transform.apply(clamped);
    let offset = center - surface;
    let distance = offset.length();
    let normal = offset.scale(W::one() / distance);
    let radius = b.radius + a.radius;
    let separation = distance - radius;
    let deepest = normal.mul_add(-radius, center);
    BatchManifold {
        normal,
        point: (surface + deepest).scale(W::splat(0.5)) + normal.scale(a.radius),
        separation,
        touching: separation.simd_le(speculative),
        fallback: distance.simd_le(W::splat(SHALLOW)) & separation.simd_le(speculative),
    }
}
