use rikka_physics_core::BitSet;
use rikka_physics_geometry::{Manifold, collide};
use rikka_physics_math::Mat3;

use super::context::KernelContext;

#[inline]
pub(crate) fn collide_one(id: u32, ctx: &KernelContext<'_>, changed: &mut BitSet) {
    let header = ctx.header(id);
    let (shape_a, shape_b) = ctx.shapes(header);
    let (frame_a, frame_b) = ctx.frames(header);
    let relative = frame_a.transform.inverse_mul(&frame_b.transform);
    let speculative = ctx.speculative_for(frame_a, frame_b);
    let local = collide(
        shape_a,
        shape_b,
        relative,
        speculative,
        &mut ctx.contact(id).cache,
    );
    let rotation = Mat3::from_quat(frame_a.transform.q);
    let mut world = Manifold {
        normal: rotation * local.normal,
        material: local.material,
        ..Default::default()
    };
    for point in local.points() {
        let mut point = *point;
        point.point = rotation * point.point + frame_a.transform.p;
        world.push(point);
    }
    ctx.complete(id, &world, changed);
}
