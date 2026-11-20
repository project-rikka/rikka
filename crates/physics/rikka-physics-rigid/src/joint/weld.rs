use rikka_physics_math::{QuatW, Vec3W, Wide};

use super::builder::Joint;
use super::impulses::{IMPULSE_SLOTS, JointImpulses, put_vec3};
use super::kinds::JointParams;
use super::math::{
    apply_angular, apply_linear, delta_rotation, inverse_mul, point_mass, skew, to_mat,
};
use super::model::{JointFrameW, JointModel, JointPrepareContext, JointSolveContext};
use super::spring::Spring;
use crate::BodyId;
use crate::solver::lanes::{BodyW, MAX_LANES};
use crate::solver::sym::SymW;

const LINEAR: usize = 0;
const ANGULAR: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weld {
    linear_spring: Option<Spring>,
    angular_spring: Option<Spring>,
}

impl Weld {
    pub fn new(body_a: BodyId, body_b: BodyId) -> Joint<Weld> {
        Joint::new(
            body_a,
            body_b,
            Weld {
                linear_spring: None,
                angular_spring: None,
            },
        )
    }

    pub fn linear_spring(&self) -> Option<Spring> {
        self.linear_spring
    }

    pub fn angular_spring(&self) -> Option<Spring> {
        self.angular_spring
    }

    pub(crate) fn is_valid(&self) -> bool {
        self.linear_spring.is_none_or(|s| s.is_valid())
            && self.angular_spring.is_none_or(|s| s.is_valid())
    }
}

impl Joint<Weld> {
    pub fn linear_spring(mut self, spring: Spring) -> Self {
        self.params.linear_spring = Some(spring);
        self
    }

    pub fn angular_spring(mut self, spring: Spring) -> Self {
        self.params.angular_spring = Some(spring);
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WeldBlock<W: Wide> {
    linear_impulse: Vec3W<W>,
    angular_impulse: Vec3W<W>,
    angular_inverse: SymW<W>,
    linear_bias: W,
    linear_mass_scale: W,
    linear_impulse_scale: W,
    angular_bias: W,
    angular_mass_scale: W,
    angular_impulse_scale: W,
    always_soft: bool,
}

#[derive(Debug)]
pub(crate) struct WeldModel;

impl JointModel for WeldModel {
    type Params = Weld;
    type Block<W: Wide> = WeldBlock<W>;

    fn params(all: &JointParams) -> &Weld {
        match all {
            JointParams::Weld(params) => params,
            _ => unreachable!("joint batch mixes kinds"),
        }
    }

    fn layout(params: &Weld) -> u32 {
        (params.linear_spring.is_some() || params.angular_spring.is_some()) as u32
    }

    #[inline(always)]
    fn prepare<W: Wide>(
        block: &mut WeldBlock<W>,
        frame: &JointFrameW<W>,
        params: &[&Weld; MAX_LANES],
        impulses: &[&JointImpulses; MAX_LANES],
        ctx: &JointPrepareContext,
    ) {
        let inertia_sum = frame.inv_i_a.add(&frame.inv_i_b);
        let linear_mass = |lane: usize| {
            let m = frame.inv_mass_a.lane(lane) + frame.inv_mass_b.lane(lane);
            if m > 0.0 { 1.0 / m } else { 0.0 }
        };
        let angular_mass = |lane: usize| {
            let trace =
                inertia_sum.xx.lane(lane) + inertia_sum.yy.lane(lane) + inertia_sum.zz.lane(lane);
            if trace > 0.0 { 3.0 / trace } else { 0.0 }
        };
        let linear = |lane: usize| {
            params[lane]
                .linear_spring
                .map(|s| s.softness(linear_mass(lane), ctx.h, ctx.max_spring_hertz))
        };
        let angular = |lane: usize| {
            params[lane]
                .angular_spring
                .map(|s| s.softness(angular_mass(lane), ctx.h, ctx.max_spring_hertz))
        };
        let pick = |soft: Option<crate::solver::softness::Softness>, lane: usize| {
            soft.unwrap_or(crate::solver::softness::Softness {
                bias_rate: frame.bias_rate.lane(lane),
                mass_scale: frame.mass_scale.lane(lane),
                impulse_scale: frame.impulse_scale.lane(lane),
            })
        };
        let warm = W::splat(ctx.warm_start);
        *block = WeldBlock {
            linear_impulse: Vec3W::from_fn(|lane| impulses[lane].vec3(LINEAR)).scale(warm),
            angular_impulse: Vec3W::from_fn(|lane| impulses[lane].vec3(ANGULAR)).scale(warm),
            angular_inverse: inertia_sum.inverse(),
            linear_bias: W::from_fn(|lane| pick(linear(lane), lane).bias_rate),
            linear_mass_scale: W::from_fn(|lane| pick(linear(lane), lane).mass_scale),
            linear_impulse_scale: W::from_fn(|lane| pick(linear(lane), lane).impulse_scale),
            angular_bias: W::from_fn(|lane| pick(angular(lane), lane).bias_rate),
            angular_mass_scale: W::from_fn(|lane| pick(angular(lane), lane).mass_scale),
            angular_impulse_scale: W::from_fn(|lane| pick(angular(lane), lane).impulse_scale),
            always_soft: Self::layout(params[0]) != 0,
        };
    }

    #[inline(always)]
    fn warm_start<W: Wide>(
        frame: &JointFrameW<W>,
        block: &WeldBlock<W>,
        a: &mut BodyW<W>,
        b: &mut BodyW<W>,
    ) {
        let r_a = a.dq.rotate(frame.anchor_a);
        let r_b = b.dq.rotate(frame.anchor_b);
        apply_angular(frame, a, b, block.angular_impulse);
        apply_linear(frame, a, b, r_a, r_b, block.linear_impulse);
    }

    #[inline(always)]
    fn solve<W: Wide>(
        frame: &JointFrameW<W>,
        block: &mut WeldBlock<W>,
        a: &mut BodyW<W>,
        b: &mut BodyW<W>,
        ctx: &JointSolveContext,
    ) {
        let zero = W::default();
        let one = W::one();
        let r_a = a.dq.rotate(frame.anchor_a);
        let r_b = b.dq.rotate(frame.anchor_b);
        let quat_a = a.dq * frame.rotation_a;
        let quat_b = b.dq * frame.rotation_b;

        let soft = ctx.use_bias || block.always_soft;
        let (linear_rhs, angular_rhs, linear_scales, angular_scales) = if soft {
            let separation = (b.dp - a.dp) + (r_b - r_a) + frame.delta_center;
            let relative = inverse_mul(quat_a, quat_b);
            let error = -quat_a.rotate(delta_rotation(relative, QuatW::identity()));
            (
                separation.scale(block.linear_bias),
                error.scale(block.angular_bias),
                (block.linear_mass_scale, block.linear_impulse_scale),
                (block.angular_mass_scale, block.angular_impulse_scale),
            )
        } else {
            (Vec3W::zero(), Vec3W::zero(), (one, zero), (one, zero))
        };
        let cdot_linear = (b.v + b.w.cross(r_b)) - (a.v + a.w.cross(r_a));
        let cdot_angular = b.w - a.w;
        let p = cdot_linear + linear_rhs;
        let q = cdot_angular + angular_rhs;

        let k_ll = point_mass(frame, r_a, r_b);
        let coupling =
            skew(r_a).mul_mat(&to_mat(&frame.inv_i_a)) + skew(r_b).mul_mat(&to_mat(&frame.inv_i_b));
        let coupling = negate(&coupling);
        let a_inv = to_mat(&block.angular_inverse);
        let ba = coupling.mul_mat(&a_inv);
        let schur = k_ll + negate(&ba.mul_mat(&coupling.transpose()));
        let x = schur.solve(p - ba.mul_vec(q));
        let y = a_inv.mul_vec(q - coupling.transpose().mul_vec(x));

        let linear = x.scale(-linear_scales.0) - block.linear_impulse.scale(linear_scales.1);
        let angular = y.scale(-angular_scales.0) - block.angular_impulse.scale(angular_scales.1);
        block.linear_impulse += linear;
        block.angular_impulse += angular;
        apply_angular(frame, a, b, angular);
        apply_linear(frame, a, b, r_a, r_b, linear);
    }

    #[inline(always)]
    fn impulses<W: Wide>(block: &WeldBlock<W>) -> [W; IMPULSE_SLOTS] {
        let mut out = [W::default(); IMPULSE_SLOTS];
        put_vec3(&mut out, LINEAR, block.linear_impulse);
        put_vec3(&mut out, ANGULAR, block.angular_impulse);
        out
    }

    fn reaction(impulses: &JointImpulses, inv_h: f32) -> (f32, f32) {
        (
            impulses.vec3(LINEAR).length() * inv_h,
            impulses.vec3(ANGULAR).length() * inv_h,
        )
    }
}

#[inline(always)]
fn negate<W: Wide>(m: &rikka_physics_math::Mat3W<W>) -> rikka_physics_math::Mat3W<W> {
    rikka_physics_math::Mat3W::from_cols(-m.x_axis, -m.y_axis, -m.z_axis)
}

impl rikka_file_format::Codec for Weld {
    type Error = crate::Error;

    fn encode(&self, out: &mut rikka_file_format::Writer<'_>) {
        out.put(&self.linear_spring);
        out.put(&self.angular_spring);
    }

    fn decode(input: &mut rikka_file_format::Reader<'_>) -> crate::Result<Self> {
        Ok(Self {
            linear_spring: input.get::<Option<Spring>>()?,
            angular_spring: input.get::<Option<Spring>>()?,
        })
    }
}
