use rikka_physics_core::Shared;
use rikka_physics_math::{MaskW, Mat3W, QuatW, Vec3, Vec3W, Wide};

use super::lanes::{LaneIndices, MAX_LANES, States, transpose};
use super::state::{
    DYNAMIC, FAST_ROTATION, LOCK_ANGULAR_X, LOCK_ANGULAR_Y, LOCK_ANGULAR_Z, LOCK_LINEAR_X,
    LOCK_LINEAR_Y, LOCK_LINEAR_Z, LOCKS, SPEED_CAPPED,
};
use super::sym::SymW;
use crate::set::{BodyMotion, BodyPose};

const _: () = {
    assert!(size_of::<BodyMotion>() == 29 * size_of::<f32>());
    assert!(std::mem::offset_of!(BodyMotion, inv_mass) == 4);
    assert!(std::mem::offset_of!(BodyMotion, inv_inertia) == 8);
    assert!(std::mem::offset_of!(BodyMotion, inertia_local) == 32);
    assert!(std::mem::offset_of!(BodyMotion, gravity_scale) == 80);
    assert!(std::mem::offset_of!(BodyMotion, linear_damping) == 84);
    assert!(std::mem::offset_of!(BodyMotion, angular_damping) == 88);
    assert!(std::mem::offset_of!(BodyMotion, force) == 92);
    assert!(std::mem::offset_of!(BodyMotion, torque) == 104);
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct IntegrateContext<'a> {
    pub states: States<'a>,
    pub motion: Shared<'a, BodyMotion>,
    pub poses: Shared<'a, BodyPose>,
    pub gravity: Vec3,
    pub h: f32,
    pub max_linear_speed: f32,
    pub max_angular_speed: f32,
}

#[inline(always)]
fn lanes_of(chunk: &[u32]) -> LaneIndices {
    let mut indices = [0u32; MAX_LANES];
    indices[..chunk.len()].copy_from_slice(chunk);
    indices
}

#[inline(always)]
fn damping(coefficient: f32, h: f32) -> f32 {
    if coefficient == 0.0 {
        1.0
    } else {
        (-coefficient * h).exp()
    }
}

#[inline(always)]
fn apply_damping<W: Wide>(vec: Vec3W<W>, coeff: W, active: usize, h: f32) -> Vec3W<W> {
    if coeff.simd_ne(W::default()).bits() == 0 {
        return vec;
    }
    let mut raw = [0.0f32; MAX_LANES];
    coeff.store(&mut raw[..W::LANES]);
    let mut factors = [1.0f32; MAX_LANES];
    for lane in 0..active {
        factors[lane] = damping(raw[lane], h);
    }
    vec.scale(W::from_fn(|lane| factors[lane]))
}

#[inline(always)]
pub(crate) fn integrate_velocities<W: Wide>(solver_indices: &[u32], ctx: &IntegrateContext<'_>) {
    static ZERO_MOTION: [f32; 29] = [0.0; 29];
    static IDENTITY_QUAT: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
    let zero = W::default();
    let h = W::splat(ctx.h);
    let gravity = Vec3W::<W>::splat(ctx.gravity);
    for chunk in solver_indices.chunks(W::LANES) {
        let indices = lanes_of(chunk);
        let kinds = ctx.states.kinds::<W>(&indices);
        let mut body = ctx.states.gather_kinds::<W>(&indices, kinds);
        let motion_rows: [*const f32; MAX_LANES] = std::array::from_fn(|lane| {
            let index = indices[lane];
            if lane < chunk.len() && index != 0 {
                std::ptr::from_ref(unsafe { ctx.motion.at(index as usize - 1) }).cast()
            } else {
                ZERO_MOTION.as_ptr()
            }
        });
        let [inv_mass, i_xx, i_xy, i_xz] = unsafe { transpose::<W>(&motion_rows, 1) };
        let [raw_gravity, lin_damp, ang_damp, fx] = unsafe { transpose::<W>(&motion_rows, 20) };
        let [_, fy, fz, tx] = unsafe { transpose::<W>(&motion_rows, 23) };
        let [_, _, ty, tz] = unsafe { transpose::<W>(&motion_rows, 25) };

        let dynamic = inv_mass.simd_gt(zero);
        let gravity_scale = W::select(dynamic, raw_gravity, zero);
        let force = Vec3W::new(fx, fy, fz);
        let torque = Vec3W::new(tx, ty, tz);

        let dv = force.scale(h * inv_mass) + gravity.scale(h * gravity_scale);
        let v = apply_damping(body.v, lin_damp, chunk.len(), ctx.h) + dv;
        let mut w = apply_damping(body.w, ang_damp, chunk.len(), ctx.h);
        if (tx.simd_ne(zero) | ty.simd_ne(zero) | tz.simd_ne(zero)).bits() != 0 {
            let [_, i_yy, i_yz, i_zz] = unsafe { transpose::<W>(&motion_rows, 4) };
            let inv_inertia = SymW {
                xx: i_xx,
                xy: i_xy,
                xz: i_xz,
                yy: i_yy,
                yz: i_yz,
                zz: i_zz,
            };
            w += inv_inertia.mul(torque).scale(h);
        }

        body.v = v;
        if (w.length_squared().simd_gt(zero) & dynamic).bits() != 0 {
            let [i00, i01, i02, i11] = unsafe { transpose::<W>(&motion_rows, 8) };
            let [_, _, i12, i22] = unsafe { transpose::<W>(&motion_rows, 10) };
            let inertia = SymW {
                xx: i00,
                xy: i01,
                xz: i02,
                yy: i11,
                yz: i12,
                zz: i22,
            };
            let pose_rows: [*const f32; MAX_LANES] = std::array::from_fn(|lane| {
                let index = indices[lane];
                if lane < chunk.len() && index != 0 {
                    std::ptr::from_ref(&unsafe { ctx.poses.at(index as usize - 1) }.transform.q)
                        .cast()
                } else {
                    IDENTITY_QUAT.as_ptr()
                }
            });
            let [qx, qy, qz, qs] = unsafe { transpose::<W>(&pose_rows, 0) };
            let q0 = QuatW::new(Vec3W::new(qx, qy, qz), qs);
            let q = body.dq * q0;
            let omega = q.rotate_inverse(w);
            let i_omega = inertia.mul(omega);
            let b = omega.cross(i_omega).scale(h);
            let (w1, w2, w3) = (omega.x, omega.y, omega.z);
            let (iw1, iw2, iw3) = (i_omega.x, i_omega.y, i_omega.z);
            let jacobian = Mat3W::from_cols(
                Vec3W::new(
                    i00 + h * (w2 * i02 - w3 * i01),
                    i01 + h * (w3 * i00 - w1 * i02 - iw3),
                    i02 + h * (w1 * i01 - w2 * i00 + iw2),
                ),
                Vec3W::new(
                    i01 + h * (w2 * i12 - w3 * i11 + iw3),
                    i11 + h * (w3 * i01 - w1 * i12),
                    i12 + h * (w1 * i11 - w2 * i01 - iw1),
                ),
                Vec3W::new(
                    i02 + h * (w2 * i22 - w3 * i12 - iw2),
                    i12 + h * (w3 * i02 - w1 * i22 + iw1),
                    i22 + h * (w1 * i12 - w2 * i02),
                ),
            );
            let omega2 = omega - jacobian.solve(b);
            let gyro = q.rotate(omega2);
            body.w = Vec3W::select(dynamic, gyro, body.w);
        } else {
            body.w = Vec3W::select(dynamic, w, body.w);
        }
        unsafe { ctx.states.scatter_kinds(&indices, kinds, &body.v, &body.w) };
    }
}

#[inline(always)]
fn has_flag<W: Wide>(flags: &[u32; MAX_LANES], bit: u32) -> W::Mask {
    W::from_fn(|lane| if flags[lane] & bit != 0 { 1.0 } else { 0.0 }).simd_gt(W::default())
}

#[inline(always)]
pub(crate) fn integrate_positions<W: Wide>(solver_indices: &[u32], ctx: &IntegrateContext<'_>) {
    let zero = W::default();
    let max_linear = W::splat(ctx.max_linear_speed);
    let max_linear_sq = max_linear * max_linear;
    let max_angular = W::splat(ctx.max_angular_speed);
    let max_angular_sq = max_angular * max_angular;
    let h = W::splat(ctx.h);
    for chunk in solver_indices.chunks(W::LANES) {
        let indices = lanes_of(chunk);
        let mut body = ctx.states.gather::<W>(&indices);
        let mut flags = [0u32; MAX_LANES];
        for lane in 0..chunk.len() {
            flags[lane] = unsafe { ctx.states.state(indices[lane]) }.flags;
        }
        let any_lock = flags.iter().any(|flag| flag & LOCKS != 0);
        let mut v = body.v;
        let mut w = body.w;
        if any_lock {
            v.x = W::select(has_flag::<W>(&flags, LOCK_LINEAR_X), zero, v.x);
            v.y = W::select(has_flag::<W>(&flags, LOCK_LINEAR_Y), zero, v.y);
            v.z = W::select(has_flag::<W>(&flags, LOCK_LINEAR_Z), zero, v.z);
            w.x = W::select(has_flag::<W>(&flags, LOCK_ANGULAR_X), zero, w.x);
            w.y = W::select(has_flag::<W>(&flags, LOCK_ANGULAR_Y), zero, w.y);
            w.z = W::select(has_flag::<W>(&flags, LOCK_ANGULAR_Z), zero, w.z);
        }
        let dynamic = has_flag::<W>(&flags, DYNAMIC);
        let speed_squared = v.length_squared();
        let linear_capped = speed_squared.simd_gt(max_linear_sq) & dynamic;
        if linear_capped.bits() != 0 {
            v = Vec3W::select(linear_capped, v.scale(max_linear / speed_squared.sqrt()), v);
        }
        let spin_squared = w.length_squared();
        let angular_capped =
            spin_squared.simd_gt(max_angular_sq) & dynamic & !has_flag::<W>(&flags, FAST_ROTATION);
        if angular_capped.bits() != 0 {
            w = Vec3W::select(
                angular_capped,
                w.scale(max_angular / spin_squared.sqrt()),
                w,
            );
        }
        body.v = v;
        body.w = w;
        body.dp = v.mul_add(h, body.dp);
        body.dq = body.dq.integrate(w, h);
        let capped = (linear_capped | angular_capped).bits();
        unsafe { ctx.states.scatter_all(&indices, &body) };
        if capped != 0 {
            for (lane, &index) in indices.iter().take(chunk.len()).enumerate() {
                if capped & (1 << lane) != 0 {
                    unsafe { ctx.states.state(index) }.flags |= SPEED_CAPPED;
                }
            }
        }
    }
}
