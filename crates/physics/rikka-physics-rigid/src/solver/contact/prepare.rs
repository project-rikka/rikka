use rikka_physics_collision::{ContactManifold, ContactPoint, MixedMaterial};
use rikka_physics_math::{MaskW, Vec3W, Wide};

use super::block::{ContactBlock, ContactLanes, ContactPointW, NO_CONTACT};
use super::context::ContactContext;
use super::manifold_ref::{contact_of, manifold};
use crate::solver::lanes::{MAX_LANES, load_masses, transpose};
use crate::solver::sym::SymW;

const MIN_FRICTION_WEIGHT: f32 = 1e-10;

const _: () = {
    assert!(std::mem::offset_of!(ContactManifold, normal) == 0);
    assert!(std::mem::offset_of!(ContactManifold, count) == 12);
    assert!(std::mem::offset_of!(MixedMaterial, friction) == 0);
    assert!(std::mem::offset_of!(MixedMaterial, restitution) == 4);
    assert!(std::mem::offset_of!(MixedMaterial, rolling_resistance) == 8);
    assert!(std::mem::offset_of!(MixedMaterial, tangent_velocity) == 12);
    assert!(
        std::mem::offset_of!(ContactManifold, twist_impulse)
            == std::mem::offset_of!(ContactManifold, friction_impulse) + 12
    );
    assert!(
        std::mem::offset_of!(ContactManifold, rolling_impulse)
            == std::mem::offset_of!(ContactManifold, twist_impulse) + 4
    );
    assert!(std::mem::offset_of!(ContactPoint, anchor_a) == 0);
    assert!(std::mem::offset_of!(ContactPoint, anchor_b) == 12);
    assert!(std::mem::offset_of!(ContactPoint, separation) == 36);
    assert!(std::mem::offset_of!(ContactPoint, normal_impulse) == 44);
};

#[inline(always)]
pub(crate) fn prepare<W: Wide>(
    block: &mut ContactBlock<W>,
    lanes: &mut ContactLanes,
    ids: &[u32],
    ctx: &ContactContext<'_>,
) {
    let empty = ContactManifold::default();
    let mut manifolds: [&ContactManifold; MAX_LANES] = [&empty; MAX_LANES];
    *lanes = ContactLanes::EMPTY;
    let mut point_count = 0u32;
    for (lane, &reference) in ids.iter().enumerate().take(W::LANES) {
        let record = &ctx.contacts[contact_of(reference) as usize];
        let manifold = unsafe { &*manifold(ctx, reference) };
        manifolds[lane] = manifold;
        lanes.contacts[lane] = reference;
        lanes.index_a[lane] = record.solver_a;
        lanes.index_b[lane] = record.solver_b;
        point_count = point_count.max(manifold.count);
    }
    lanes.point_count = point_count;
    lanes.contacts[ids.len().min(W::LANES)..W::LANES].fill(NO_CONTACT);
    lanes.kinds_a = ctx.states.kinds::<W>(&lanes.index_a);
    lanes.kinds_b = ctx.states.kinds::<W>(&lanes.index_b);

    let zero = W::default();
    let one = W::one();
    let (m_a, i_a) = load_masses::<W>(ctx.motion, ctx.split, &lanes.index_a);
    let (m_b, i_b) = load_masses::<W>(ctx.motion, ctx.split, &lanes.index_b);
    block.inv_mass_a = m_a;
    block.inv_mass_b = m_b;
    block.inv_i_a = i_a;
    block.inv_i_b = i_b;

    let static_mask = m_a.simd_eq(zero) | m_b.simd_eq(zero);
    block.bias_rate = W::select(
        static_mask,
        W::splat(ctx.static_soft.mass_scale * ctx.static_soft.bias_rate),
        W::splat(ctx.soft.mass_scale * ctx.soft.bias_rate),
    );
    block.mass_scale = W::select(
        static_mask,
        W::splat(ctx.static_soft.mass_scale),
        W::splat(ctx.soft.mass_scale),
    );
    block.impulse_scale = W::select(
        static_mask,
        W::splat(ctx.static_soft.impulse_scale),
        W::splat(ctx.soft.impulse_scale),
    );

    let normal_rows = manifolds.map(|m| std::ptr::from_ref(m).cast::<f32>());
    let [nx, ny, nz, _] = unsafe { transpose::<W>(&normal_rows, 0) };
    let normal = Vec3W::new(nx, ny, nz);

    let material_rows = manifolds.map(|m| std::ptr::from_ref(&m.material).cast::<f32>());
    let [friction, restitution, rolling_resistance, tvx] =
        unsafe { transpose::<W>(&material_rows, 0) };
    let [_, _, tvy, tvz] = unsafe { transpose::<W>(&material_rows, 2) };
    let tangent_velocity = Vec3W::new(tvx, tvy, tvz);
    block.friction = friction;
    block.restitution = restitution;
    block.rolling_resistance = rolling_resistance;

    let impulse_rows = manifolds.map(|m| std::ptr::from_ref(&m.friction_impulse).cast::<f32>());
    let [fx, fy, fz, twist_impulse] = unsafe { transpose::<W>(&impulse_rows, 0) };
    let [_, rx, ry, rz] = unsafe { transpose::<W>(&impulse_rows, 3) };
    let friction_impulse = Vec3W::new(fx, fy, fz);
    let rolling_impulse = Vec3W::new(rx, ry, rz);

    let rolling_mask = rolling_resistance.simd_gt(zero);
    lanes.rolling = rolling_mask.bits() != 0;
    lanes.restitution = restitution.simd_gt(zero).bits() != 0;

    let tangent1 = normal.any_orthogonal().normalize_or_zero();
    let tangent2 = tangent1.cross(normal);
    block.normal = normal;
    block.tangent1 = tangent1;
    block.tangent2 = tangent2;
    block.tangent_velocity1 = tangent_velocity.dot(tangent1);
    block.tangent_velocity2 = tangent_velocity.dot(tangent2);

    let warm = W::splat(ctx.warm_start);
    block.twist_impulse = warm * twist_impulse;
    block.rolling_impulse = Vec3W::select(rolling_mask, rolling_impulse.scale(warm), Vec3W::zero());
    block.friction_impulse1 = warm * friction_impulse.dot(tangent1);
    block.friction_impulse2 = warm * friction_impulse.dot(tangent2);

    let counts = W::from_fn(|lane| manifolds[lane].count as f32);
    let inv_tau = W::splat(1.0 / ctx.speculative);
    let min_weight = W::splat(MIN_FRICTION_WEIGHT);
    let two = W::splat(2.0);
    let mut center_a = Vec3W::<W>::zero();
    let mut center_b = Vec3W::<W>::zero();
    let mut total_weight = zero;

    let velocity_a = ctx.states.gather_velocity::<W>(&lanes.index_a);
    let velocity_b = ctx.states.gather_velocity::<W>(&lanes.index_b);

    let point_count = lanes.point_count as usize;
    for (index, point) in block.points[..point_count].iter_mut().enumerate() {
        let mask = counts.simd_gt(W::splat(index as f32));
        let rows = manifolds.map(|m| std::ptr::from_ref(&m.points[index]).cast::<f32>());
        let [ax, ay, az, bx] = unsafe { transpose::<W>(&rows, 0) };
        let [_, by, bz, _] = unsafe { transpose::<W>(&rows, 3) };
        let [raw_separation, _, raw_impulse, _] = unsafe { transpose::<W>(&rows, 9) };
        let r_a = Vec3W::select(mask, Vec3W::new(ax, ay, az), Vec3W::zero());
        let r_b = Vec3W::select(mask, Vec3W::new(bx, by, bz), Vec3W::zero());
        let separation = W::select(mask, raw_separation, zero);
        let impulse = W::select(mask, raw_impulse, zero);
        let weight = (two - separation * inv_tau).max(min_weight).min(one);
        let weight = W::select(mask, weight, zero);
        center_a = r_a.mul_add(weight, center_a);
        center_b = r_b.mul_add(weight, center_b);
        total_weight += weight;

        let rn_a = r_a.cross(normal);
        let rn_b = r_b.cross(normal);
        let k = m_a + m_b + rn_a.dot(i_a.mul(rn_a)) + rn_b.dot(i_b.mul(rn_b));
        let valid = k.simd_gt(zero) & mask;

        let vr_a = velocity_a.v + velocity_a.w.cross(r_a);
        let vr_b = velocity_b.v + velocity_b.w.cross(r_b);
        *point = ContactPointW {
            anchor_a: r_a,
            anchor_b: r_b,
            base_separation: separation - (r_b - r_a).dot(normal),
            normal_impulse: warm * impulse,
            total_normal_impulse: zero,
            normal_mass: W::select(valid, one / k, zero),
            lever_arm: zero,
            relative_velocity: W::select(mask, normal.dot(vr_b - vr_a), zero),
            restitution_impulse: zero,
        };
    }

    let inv_weight = W::select(total_weight.simd_gt(zero), one / total_weight, zero);
    let center_a = center_a.scale(inv_weight);
    let center_b = center_b.scale(inv_weight);
    block.center_a = center_a;
    block.center_b = center_b;
    for (index, point) in block.points[..point_count].iter_mut().enumerate() {
        let mask = counts.simd_gt(W::splat(index as f32));
        let d = point.anchor_a - center_a;
        point.lever_arm = W::select(mask, d.length(), zero);
    }

    let rt_a1 = center_a.cross(tangent1);
    let rt_a2 = center_a.cross(tangent2);
    let rt_b1 = center_b.cross(tangent1);
    let rt_b2 = center_b.cross(tangent2);
    let kxx = m_a + m_b + rt_a1.dot(i_a.mul(rt_a1)) + rt_b1.dot(i_b.mul(rt_b1));
    let kyy = m_a + m_b + rt_a2.dot(i_a.mul(rt_a2)) + rt_b2.dot(i_b.mul(rt_b2));
    let kxy = rt_a1.dot(i_a.mul(rt_a2)) + rt_b1.dot(i_b.mul(rt_b2));
    let det = kxx * kyy - kxy * kxy;
    let valid = det.abs().simd_gt(W::splat(1000.0 * f32::MIN_POSITIVE));
    let inv_det = W::select(valid, one / det, zero);
    block.tangent_mass_xx = inv_det * kyy;
    block.tangent_mass_xy = -(inv_det * kxy);
    block.tangent_mass_yy = inv_det * kxx;

    let i_ab = i_a.add(&i_b);
    let k_twist = normal.dot(i_ab.mul(normal));
    block.twist_mass = W::select(k_twist.simd_gt(zero), one / k_twist, zero);
    block.rolling_mass = if lanes.rolling {
        i_ab.inverse()
    } else {
        SymW::zero()
    };
}
