use rikka_physics_core::BitSet;
use rikka_physics_geometry::{
    BatchManifold, Capsule, CapsuleLanes, Cuboid, CuboidLanes, Shape, Sphere, SphereLanes,
    TransformLanes, collide_batch_capsule_sphere, collide_batch_capsules,
    collide_batch_cuboid_sphere, collide_batch_spheres,
};
use rikka_physics_math::{Transform, Vec3, Wide};

use super::context::KernelContext;
use super::general;

const IDLE_OFFSET: Vec3 = Vec3::new(4.0, 0.0, 0.0);

#[derive(Clone, Copy)]
enum Side {
    Forward,
    Reversed,
}

struct Frames<W: Wide> {
    relative: TransformLanes<W>,
    base: TransformLanes<W>,
    speculative: W,
}

#[inline(always)]
fn frames<W: Wide>(ids: &[u32], ctx: &KernelContext<'_>, side: Side) -> Frames<W> {
    let mut relative = [Transform::from_translation(IDLE_OFFSET); 8];
    let mut base = [Transform::IDENTITY; 8];
    let mut speculative = [0.0f32; 8];
    for (lane, &id) in ids.iter().enumerate() {
        let (a, b) = ctx.frames(ctx.header(id));
        let (first, second) = match side {
            Side::Forward => (a, b),
            Side::Reversed => (b, a),
        };
        relative[lane] = first.transform.inverse_mul(&second.transform);
        base[lane] = first.transform;
        speculative[lane] = ctx.speculative_for(a, b);
    }
    Frames {
        relative: TransformLanes::from_fn(|lane| relative[lane]),
        base: TransformLanes::from_fn(|lane| base[lane]),
        speculative: W::from_fn(|lane| speculative[lane]),
    }
}

#[inline(always)]
fn shape_of<'a>(
    ids: &[u32],
    ctx: &KernelContext<'a>,
    lane: usize,
    side: Side,
) -> Option<&'a Shape> {
    let id = *ids.get(lane)?;
    let (a, b) = ctx.shapes(ctx.header(id));
    Some(match side {
        Side::Forward => a,
        Side::Reversed => b,
    })
}

#[inline(always)]
fn spheres<W: Wide>(ids: &[u32], ctx: &KernelContext<'_>, side: Side) -> SphereLanes<W> {
    SphereLanes::from_fn(|lane| match shape_of(ids, ctx, lane, side) {
        Some(Shape::Sphere(sphere)) => *sphere,
        _ => Sphere::with_center(Vec3::ZERO, 1.0),
    })
}

#[inline(always)]
fn capsules<W: Wide>(ids: &[u32], ctx: &KernelContext<'_>, side: Side) -> CapsuleLanes<W> {
    CapsuleLanes::from_fn(|lane| match shape_of(ids, ctx, lane, side) {
        Some(Shape::Capsule(capsule)) => *capsule,
        _ => Capsule::segment(Vec3::ZERO, Vec3::Y, 1.0),
    })
}

#[inline(always)]
fn cuboids<W: Wide>(ids: &[u32], ctx: &KernelContext<'_>, side: Side) -> CuboidLanes<W> {
    CuboidLanes::from_fn(|lane| match shape_of(ids, ctx, lane, side) {
        Some(Shape::Cuboid(cuboid)) => *cuboid,
        _ => Cuboid::new(Vec3::ONE),
    })
}

#[inline(always)]
fn emit<W: Wide>(
    ids: &[u32],
    batch: BatchManifold<W>,
    frames: &Frames<W>,
    side: Side,
    ctx: &KernelContext<'_>,
    changed: &mut BitSet,
) {
    let normal = frames.base.q.rotate(batch.normal);
    let world = BatchManifold {
        normal: match side {
            Side::Forward => normal,
            Side::Reversed => -normal,
        },
        point: frames.base.apply(batch.point),
        ..batch
    };
    for (lane, &id) in ids.iter().enumerate() {
        match world.manifold(lane) {
            Some(manifold) => ctx.complete(id, &manifold, changed),
            None => general::collide_one(id, ctx, changed),
        }
    }
}

#[inline(always)]
pub(crate) fn sphere_sphere<W: Wide>(ids: &[u32], ctx: &KernelContext<'_>, changed: &mut BitSet) {
    let side = Side::Forward;
    let frames = frames::<W>(ids, ctx, side);
    let batch = collide_batch_spheres(
        &spheres(ids, ctx, Side::Forward),
        &spheres(ids, ctx, Side::Reversed),
        &frames.relative,
        frames.speculative,
    );
    emit(ids, batch, &frames, side, ctx, changed);
}

#[inline(always)]
pub(crate) fn sphere_capsule<W: Wide>(ids: &[u32], ctx: &KernelContext<'_>, changed: &mut BitSet) {
    let side = Side::Reversed;
    let frames = frames::<W>(ids, ctx, side);
    let batch = collide_batch_capsule_sphere(
        &capsules(ids, ctx, Side::Reversed),
        &spheres(ids, ctx, Side::Forward),
        &frames.relative,
        frames.speculative,
    );
    emit(ids, batch, &frames, side, ctx, changed);
}

#[inline(always)]
pub(crate) fn capsule_capsule<W: Wide>(ids: &[u32], ctx: &KernelContext<'_>, changed: &mut BitSet) {
    let side = Side::Forward;
    let frames = frames::<W>(ids, ctx, side);
    let batch = collide_batch_capsules(
        &capsules(ids, ctx, Side::Forward),
        &capsules(ids, ctx, Side::Reversed),
        &frames.relative,
        frames.speculative,
    );
    emit(ids, batch, &frames, side, ctx, changed);
}

#[inline(always)]
pub(crate) fn sphere_cuboid<W: Wide>(ids: &[u32], ctx: &KernelContext<'_>, changed: &mut BitSet) {
    let side = Side::Reversed;
    let frames = frames::<W>(ids, ctx, side);
    let batch = collide_batch_cuboid_sphere(
        &cuboids(ids, ctx, Side::Reversed),
        &spheres(ids, ctx, Side::Forward),
        &frames.relative,
        frames.speculative,
    );
    emit(ids, batch, &frames, side, ctx, changed);
}
