use rikka_physics_math::Wide;

use super::block::{ContactBlock, ContactLanes, NO_CONTACT};
use super::context::ContactContext;
use super::manifold_ref::manifold;

#[inline(always)]
pub(crate) fn store<W: Wide>(
    block: &ContactBlock<W>,
    lanes: &ContactLanes,
    ctx: &ContactContext<'_>,
) {
    for (lane, &id) in lanes.contacts[..W::LANES].iter().enumerate() {
        if id == NO_CONTACT {
            continue;
        }
        let manifold = unsafe { manifold(ctx, id) };
        for (index, point) in manifold.points_mut().iter_mut().enumerate() {
            let source = &block.points[index];
            point.normal_impulse = source.normal_impulse.lane(lane);
            point.total_normal_impulse = source.total_normal_impulse.lane(lane);
            point.normal_velocity = source.relative_velocity.lane(lane);
        }
        let t1 = block.tangent1.lane(lane);
        let t2 = block.tangent2.lane(lane);
        manifold.friction_impulse =
            t1 * block.friction_impulse1.lane(lane) + t2 * block.friction_impulse2.lane(lane);
        manifold.twist_impulse = block.twist_impulse.lane(lane);
        manifold.rolling_impulse = block.rolling_impulse.lane(lane);
    }
}
