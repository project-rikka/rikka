use rikka_physics_math::{Vec3W, Wide};

use super::block::{ContactBlock, ContactLanes};
use crate::solver::lanes::{BodyW, States, VelocityW};

#[derive(Clone, Copy, Debug)]
pub(crate) struct IterationContext {
    pub inv_h: f32,
    pub contact_speed: f32,
    pub restitution_threshold: f32,
}

#[inline(always)]
fn apply<W: Wide>(
    block: &ContactBlock<W>,
    a: &mut BodyW<W>,
    b: &mut BodyW<W>,
    r_a: Vec3W<W>,
    r_b: Vec3W<W>,
    p: Vec3W<W>,
) {
    a.w -= block.inv_i_a.mul(r_a.cross(p));
    a.v = p.mul_add(-block.inv_mass_a, a.v);
    b.w += block.inv_i_b.mul(r_b.cross(p));
    b.v = p.mul_add(block.inv_mass_b, b.v);
}

#[inline(always)]
pub(crate) fn warm_start<W: Wide>(
    block: &ContactBlock<W>,
    lanes: &ContactLanes,
    a: &mut VelocityW<W>,
    b: &mut VelocityW<W>,
) {
    let mut total = W::default();
    let mut moment_a = Vec3W::<W>::zero();
    let mut moment_b = Vec3W::<W>::zero();
    for point in &block.points[..lanes.point_count as usize] {
        total += point.normal_impulse;
        moment_a = point.anchor_a.mul_add(point.normal_impulse, moment_a);
        moment_b = point.anchor_b.mul_add(point.normal_impulse, moment_b);
    }
    let normal = block.normal;
    let friction = block.tangent2.mul_add(
        block.friction_impulse2,
        block.tangent1.scale(block.friction_impulse1),
    );
    let linear = normal.mul_add(total, friction);
    let twist = normal.scale(block.twist_impulse);
    let mut angular_a = moment_a.cross(normal) + block.center_a.cross(friction) + twist;
    let mut angular_b = moment_b.cross(normal) + block.center_b.cross(friction) + twist;
    if lanes.rolling {
        angular_a += block.rolling_impulse;
        angular_b += block.rolling_impulse;
    }
    a.w -= block.inv_i_a.mul(angular_a);
    a.v = linear.mul_add(-block.inv_mass_a, a.v);
    b.w += block.inv_i_b.mul(angular_b);
    b.v = linear.mul_add(block.inv_mass_b, b.v);
}

#[inline(always)]
pub(crate) fn push<W: Wide>(
    block: &mut ContactBlock<W>,
    lanes: &ContactLanes,
    a: &mut BodyW<W>,
    b: &mut BodyW<W>,
    ctx: &IterationContext,
) {
    let zero = W::default();
    let one = W::one();
    let inv_h = W::splat(ctx.inv_h);
    let max_push = W::splat(-ctx.contact_speed);
    let normal = block.normal;
    let normal_separation = normal.dot(b.dp - a.dp);
    let normal_a = a.dq.rotate_inverse(normal);
    let normal_b = b.dq.rotate_inverse(normal);
    for index in 0..lanes.point_count as usize {
        let point = &block.points[index];
        let r_a = point.anchor_a;
        let r_b = point.anchor_b;
        let s = normal_separation + normal_b.dot(r_b) - normal_a.dot(r_a) + point.base_separation;
        let separated = s.simd_gt(zero);
        let speculative = s * inv_h;
        let overlap = (block.bias_rate * s).max(max_push);
        let bias = W::select(separated, speculative, overlap);
        let mass_scale = W::select(separated, one, block.mass_scale);
        let impulse_scale = W::select(separated, zero, block.impulse_scale);
        let vr_a = a.v + a.w.cross(r_a);
        let vr_b = b.v + b.w.cross(r_b);
        let vn = (vr_b - vr_a).dot(normal);
        let old = point.normal_impulse;
        let negative = point.normal_mass * mass_scale.mul_add(vn, bias) + impulse_scale * old;
        let new = (old - negative).max(zero);
        block.points[index].normal_impulse = new;
        apply(block, a, b, r_a, r_b, normal.scale(new - old));
    }
}

#[inline(always)]
pub(crate) fn relax<W: Wide>(
    block: &mut ContactBlock<W>,
    lanes: &ContactLanes,
    a: &mut BodyW<W>,
    b: &mut BodyW<W>,
    ctx: &IterationContext,
) {
    let zero = W::default();
    let one = W::one();
    let epsilon = W::splat(f32::EPSILON);
    let inv_h = W::splat(ctx.inv_h);
    let normal = block.normal;
    let normal_separation = normal.dot(b.dp - a.dp);
    let normal_a = a.dq.rotate_inverse(normal);
    let normal_b = b.dq.rotate_inverse(normal);
    let mut total_normal = zero;
    let mut total_twist = zero;
    for index in 0..lanes.point_count as usize {
        let point = &block.points[index];
        let r_a = point.anchor_a;
        let r_b = point.anchor_b;
        let s = normal_separation + normal_b.dot(r_b) - normal_a.dot(r_a) + point.base_separation;
        let bias = (s * inv_h).max(zero);
        let vr_a = a.v + a.w.cross(r_a);
        let vr_b = b.v + b.w.cross(r_b);
        let vn = (vr_b - vr_a).dot(normal);
        let old = point.normal_impulse;
        let new = (old - point.normal_mass * (vn + bias)).max(zero);
        let lever = point.lever_arm;
        let point = &mut block.points[index];
        point.normal_impulse = new;
        point.total_normal_impulse += new;
        total_normal += new;
        total_twist = lever.mul_add(new, total_twist);
        apply(block, a, b, r_a, r_b, normal.scale(new - old));
    }

    if lanes.rolling {
        let delta = block.rolling_mass.mul(a.w - b.w);
        let old = block.rolling_impulse;
        let accumulated = old + delta;
        let max = block.rolling_resistance * total_normal;
        let length_squared = accumulated.length_squared();
        let over = length_squared.simd_gt(max * max);
        let normalize = max / (length_squared.sqrt() + epsilon);
        let scale = W::select(over, normalize, one);
        let scale = W::select(block.rolling_resistance.simd_gt(zero), scale, zero);
        block.rolling_impulse = accumulated.scale(scale);
        let delta = block.rolling_impulse - old;
        a.w -= block.inv_i_a.mul(delta);
        b.w += block.inv_i_b.mul(delta);
    }

    {
        let twist_speed = normal.dot(b.w - a.w);
        let max = block.friction * total_twist;
        let old = block.twist_impulse;
        let new = (old - block.twist_mass * twist_speed).max(-max).min(max);
        block.twist_impulse = new;
        let l = normal.scale(new - old);
        a.w -= block.inv_i_a.mul(l);
        b.w += block.inv_i_b.mul(l);
    }

    {
        let r_a = block.center_a;
        let r_b = block.center_b;
        let vr = (b.v + b.w.cross(r_b)) - (a.v + a.w.cross(r_a));
        let vt1 = vr.dot(block.tangent1) - block.tangent_velocity1;
        let vt2 = vr.dot(block.tangent2) - block.tangent_velocity2;
        let d1 = -(block.tangent_mass_xx * vt1 + block.tangent_mass_xy * vt2);
        let d2 = -(block.tangent_mass_xy * vt1 + block.tangent_mass_yy * vt2);
        let n1 = block.friction_impulse1 + d1;
        let n2 = block.friction_impulse2 + d2;
        let max = block.friction * total_normal;
        let length_squared = n1 * n1 + n2 * n2;
        let over = length_squared.simd_gt(max * max);
        let normalize = max / (length_squared.sqrt() + epsilon);
        let scale = W::select(over, normalize, one);
        let n1 = n1 * scale;
        let n2 = n2 * scale;
        let d1 = n1 - block.friction_impulse1;
        let d2 = n2 - block.friction_impulse2;
        block.friction_impulse1 = n1;
        block.friction_impulse2 = n2;
        let p = block.tangent2.mul_add(d2, block.tangent1.scale(d1));
        apply(block, a, b, r_a, r_b, p);
    }
}

#[inline(always)]
pub(crate) fn restitution<W: Wide>(
    block: &mut ContactBlock<W>,
    lanes: &ContactLanes,
    a: &mut BodyW<W>,
    b: &mut BodyW<W>,
    ctx: &IterationContext,
) {
    let zero = W::default();
    let inv_h = W::splat(ctx.inv_h);
    let threshold = W::splat(-ctx.restitution_threshold);
    let restitution_mask = block.restitution.simd_gt(zero);
    let normal = block.normal;
    let normal_separation = normal.dot(b.dp - a.dp);
    let normal_a = a.dq.rotate_inverse(normal);
    let normal_b = b.dq.rotate_inverse(normal);
    for index in 0..lanes.point_count as usize {
        let point = block.points[index];
        let normal_mass = W::select(restitution_mask, point.normal_mass, zero);
        let compression = point.total_normal_impulse - point.restitution_impulse;
        let armed = restitution_mask
            & point.relative_velocity.simd_lt(threshold)
            & compression.simd_gt(zero);
        let s = normal_separation + normal_b.dot(point.anchor_b) - normal_a.dot(point.anchor_a)
            + point.base_separation;
        let speculative = (s * inv_h).max(zero);
        let bias = W::select(
            armed,
            block.restitution * point.relative_velocity,
            speculative,
        );
        let vr_a = a.v + a.w.cross(point.anchor_a);
        let vr_b = b.v + b.w.cross(point.anchor_b);
        let vn = (vr_b - vr_a).dot(normal);
        let new = (point.normal_impulse - normal_mass * (vn + bias)).max(zero);
        let impulse = new - point.normal_impulse;
        let approach = (-(normal_mass * vn)).max(zero).min(impulse.max(zero));
        let allowance = block.restitution * (compression + approach) - point.restitution_impulse;
        let max = approach + allowance.max(zero);
        let impulse = W::select(armed, impulse.min(max), impulse);
        let target = &mut block.points[index];
        target.normal_impulse += impulse;
        target.restitution_impulse += impulse - approach;
        target.total_normal_impulse += impulse;
        apply(
            block,
            a,
            b,
            point.anchor_a,
            point.anchor_b,
            normal.scale(impulse),
        );
    }
}

#[inline(always)]
pub(crate) fn solve_block<W: Wide>(
    block: &mut ContactBlock<W>,
    lanes: &ContactLanes,
    states: &States<'_>,
    ctx: &IterationContext,
    pass: ContactPass,
) {
    if pass == ContactPass::Warm {
        let mut a = states.gather_velocity::<W>(&lanes.index_a);
        let mut b = states.gather_velocity::<W>(&lanes.index_b);
        warm_start(block, lanes, &mut a, &mut b);
        unsafe {
            states.scatter_kinds(&lanes.index_a, lanes.kinds_a, &a.v, &a.w);
            states.scatter_kinds(&lanes.index_b, lanes.kinds_b, &b.v, &b.w);
        }
        return;
    }
    let mut a = states.gather_kinds::<W>(&lanes.index_a, lanes.kinds_a);
    let mut b = states.gather_kinds::<W>(&lanes.index_b, lanes.kinds_b);
    solve_pass(block, lanes, &mut a, &mut b, ctx, pass);
    unsafe {
        states.scatter_kinds(&lanes.index_a, lanes.kinds_a, &a.v, &a.w);
        states.scatter_kinds(&lanes.index_b, lanes.kinds_b, &b.v, &b.w);
    }
}

#[inline(always)]
pub(crate) fn solve_pass<W: Wide>(
    block: &mut ContactBlock<W>,
    lanes: &ContactLanes,
    a: &mut BodyW<W>,
    b: &mut BodyW<W>,
    ctx: &IterationContext,
    pass: ContactPass,
) {
    match pass {
        ContactPass::Warm => {
            let mut va = VelocityW { v: a.v, w: a.w };
            let mut vb = VelocityW { v: b.v, w: b.w };
            warm_start(block, lanes, &mut va, &mut vb);
            (a.v, a.w, b.v, b.w) = (va.v, va.w, vb.v, vb.w);
        }
        ContactPass::Solve => push(block, lanes, a, b, ctx),
        ContactPass::Relax => relax(block, lanes, a, b, ctx),
        ContactPass::RelaxRestitution => {
            relax(block, lanes, a, b, ctx);
            if lanes.restitution {
                restitution(block, lanes, a, b, ctx);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ContactPass {
    Warm,
    Solve,
    Relax,
    RelaxRestitution,
}
