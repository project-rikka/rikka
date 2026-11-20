use std::ops::RangeInclusive;

use rikka_physics_math::{Quat, QuatW, Vec3, Vec3W, Wide};

use super::builder::Joint;
use super::impulses::{IMPULSE_SLOTS, JointImpulses, put_vec3};
use super::kinds::JointParams;
use super::math::{
    apply_angular, apply_linear, axis, delta_rotation, inverse_mul, limit_softness, point_mass,
};
use super::model::{JointFrameW, JointModel, JointPrepareContext, JointSolveContext};
use super::spring::Spring;
use super::swing_twist::{cone_angle, twist_angle};
use crate::BodyId;
use crate::solver::lanes::{BodyW, MAX_LANES};
use crate::solver::sym::SymW;

const SPRING: u32 = 1;
const MOTOR: u32 = 2;
const CONE: u32 = 4;
const TWIST: u32 = 8;

const LINEAR: usize = 0;
const SPRING_IMPULSE: usize = 3;
const MOTOR_IMPULSE: usize = 6;
const LOWER_TWIST: usize = 9;
const UPPER_TWIST: usize = 10;
const SWING: usize = 11;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spherical {
    cone: Option<(f32, f32)>,
    twist: Option<(f32, f32)>,
    spring: Option<Spring>,
    target: Quat,
    motor: Option<(Vec3, f32)>,
}

impl Spherical {
    pub fn new(body_a: BodyId, body_b: BodyId) -> Joint<Spherical> {
        Joint::new(
            body_a,
            body_b,
            Spherical {
                cone: None,
                twist: None,
                spring: None,
                target: Quat::IDENTITY,
                motor: None,
            },
        )
    }

    pub fn cone(&self) -> Option<(f32, f32)> {
        self.cone
    }

    pub fn twist(&self) -> Option<(f32, f32)> {
        self.twist
    }

    pub fn spring(&self) -> Option<Spring> {
        self.spring
    }

    pub fn target(&self) -> Quat {
        self.target
    }

    pub fn motor(&self) -> Option<(Vec3, f32)> {
        self.motor
    }

    pub(crate) fn is_valid(&self) -> bool {
        let cone = self
            .cone
            .is_none_or(|(x, y)| x.is_finite() && y.is_finite() && x >= 0.0 && y >= 0.0);
        let twist = self
            .twist
            .is_none_or(|(lower, upper)| lower.is_finite() && upper.is_finite() && lower <= upper);
        let motor = self
            .motor
            .is_none_or(|(velocity, torque)| velocity.is_finite() && torque >= 0.0);
        cone && twist
            && motor
            && self.spring.is_none_or(|s| s.is_valid())
            && self.target.is_finite()
    }
}

impl Joint<Spherical> {
    pub fn cone(self, angle: f32) -> Self {
        self.elliptic_cone(angle, angle)
    }

    pub fn elliptic_cone(mut self, angle_x: f32, angle_y: f32) -> Self {
        self.params.cone = Some((angle_x, angle_y));
        self
    }

    pub fn twist(mut self, range: RangeInclusive<f32>) -> Self {
        self.params.twist = Some((*range.start(), *range.end()));
        self
    }

    pub fn spring(mut self, spring: Spring) -> Self {
        self.params.spring = Some(spring);
        self
    }

    pub fn target(mut self, rotation: Quat) -> Self {
        self.params.target = rotation;
        self
    }

    pub fn motor(mut self, velocity: Vec3, max_torque: f32) -> Self {
        self.params.motor = Some((velocity, max_torque));
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SphericalBlock<W: Wide> {
    linear_impulse: Vec3W<W>,
    spring_impulse: Vec3W<W>,
    motor_impulse: Vec3W<W>,
    lower_twist_impulse: W,
    upper_twist_impulse: W,
    swing_impulse: W,
    rotation_mass: SymW<W>,
    swing_axis: Vec3W<W>,
    swing_mass: W,
    cone_angle: W,
    twist_jacobian: Vec3W<W>,
    twist_mass: W,
    lower_twist: W,
    upper_twist: W,
    spring_bias: W,
    spring_mass_scale: W,
    spring_impulse_scale: W,
    target: QuatW<W>,
    motor_velocity: Vec3W<W>,
    max_motor_impulse: W,
    layout: u32,
}

#[derive(Debug)]
pub(crate) struct SphericalModel;

impl JointModel for SphericalModel {
    type Params = Spherical;
    type Block<W: Wide> = SphericalBlock<W>;

    fn params(all: &JointParams) -> &Spherical {
        match all {
            JointParams::Spherical(params) => params,
            _ => unreachable!("joint batch mixes kinds"),
        }
    }

    fn layout(params: &Spherical) -> u32 {
        (params.spring.is_some() as u32 * SPRING)
            | (params.motor.is_some() as u32 * MOTOR)
            | (params.cone.is_some() as u32 * CONE)
            | (params.twist.is_some() as u32 * TWIST)
    }

    #[inline(always)]
    fn prepare<W: Wide>(
        block: &mut SphericalBlock<W>,
        frame: &JointFrameW<W>,
        params: &[&Spherical; MAX_LANES],
        impulses: &[&JointImpulses; MAX_LANES],
        ctx: &JointPrepareContext,
    ) {
        let zero = W::default();
        let one = W::one();
        let layout = Self::layout(params[0]);
        let inertia_sum = frame.inv_i_a.add(&frame.inv_i_b);
        let rotation_mass = inertia_sum.inverse();
        let cone_axis = axis(frame.rotation_a, Vec3::Z);
        let twist_axis = axis(frame.rotation_b, Vec3::Z);
        let swing_axis = cone_axis.cross(twist_axis).normalize_or_zero();
        let k_swing = swing_axis.dot(inertia_sum.mul(swing_axis));
        let swing_mass = W::select(k_swing.simd_gt(zero), one / k_swing, zero);

        let local_swing = frame.rotation_a.rotate_inverse(swing_axis);
        let cone_angle = W::from_fn(|lane| {
            let (x, y) = params[lane].cone.unwrap_or((0.0, 0.0));
            let d = local_swing.lane(lane);
            let (cx, cy) = (d.x, d.y);
            let denominator = ((y * cx) * (y * cx) + (x * cy) * (x * cy)).sqrt();
            if denominator > f32::EPSILON {
                x * y / denominator
            } else {
                x.max(y)
            }
        });

        let relative = inverse_mul(frame.rotation_a, frame.rotation_b);
        let tan_half = ((relative.v.x * relative.v.x + relative.v.y * relative.v.y)
            / (relative.v.z * relative.v.z + relative.s * relative.s))
            .sqrt();
        let perp = swing_axis.cross(cone_axis);
        let twist_jacobian = perp.mul_add(tan_half, cone_axis);
        let k_twist = twist_jacobian.dot(inertia_sum.mul(twist_jacobian));
        let twist_mass = W::select(k_twist.simd_gt(zero), one / k_twist, zero);

        let effective = |lane: usize| {
            let trace =
                inertia_sum.xx.lane(lane) + inertia_sum.yy.lane(lane) + inertia_sum.zz.lane(lane);
            if trace > 0.0 { 3.0 / trace } else { 0.0 }
        };
        let spring = |lane: usize| {
            params[lane]
                .spring
                .map(|spring| spring.softness(effective(lane), ctx.h, ctx.max_spring_hertz))
                .unwrap_or_default()
        };
        let warm = W::splat(ctx.warm_start);
        let impulse =
            |slot: usize| Vec3W::<W>::from_fn(|lane| impulses[lane].vec3(slot)).scale(warm);
        let scalar = |slot: usize| W::from_fn(|lane| impulses[lane].0[slot]) * warm;
        *block = SphericalBlock {
            linear_impulse: impulse(LINEAR),
            spring_impulse: impulse(SPRING_IMPULSE),
            motor_impulse: impulse(MOTOR_IMPULSE),
            lower_twist_impulse: scalar(LOWER_TWIST),
            upper_twist_impulse: scalar(UPPER_TWIST),
            swing_impulse: scalar(SWING),
            rotation_mass,
            swing_axis,
            swing_mass,
            cone_angle,
            twist_jacobian,
            twist_mass,
            lower_twist: W::from_fn(|lane| params[lane].twist.map_or(0.0, |t| t.0)),
            upper_twist: W::from_fn(|lane| params[lane].twist.map_or(0.0, |t| t.1)),
            spring_bias: W::from_fn(|lane| spring(lane).bias_rate),
            spring_mass_scale: W::from_fn(|lane| spring(lane).mass_scale),
            spring_impulse_scale: W::from_fn(|lane| spring(lane).impulse_scale),
            target: QuatW::from_fn(|lane| params[lane].target),
            motor_velocity: Vec3W::from_fn(|lane| params[lane].motor.map_or(Vec3::ZERO, |m| m.0)),
            max_motor_impulse: W::from_fn(|lane| params[lane].motor.map_or(0.0, |m| m.1 * ctx.h)),
            layout,
        };
    }

    #[inline(always)]
    fn warm_start<W: Wide>(
        frame: &JointFrameW<W>,
        block: &SphericalBlock<W>,
        a: &mut BodyW<W>,
        b: &mut BodyW<W>,
    ) {
        let r_a = a.dq.rotate(frame.anchor_a);
        let r_b = b.dq.rotate(frame.anchor_b);
        let angular = (block.spring_impulse + block.motor_impulse)
            .mul_add(W::one(), block.swing_axis.scale(-block.swing_impulse))
            + block
                .twist_jacobian
                .scale(block.lower_twist_impulse - block.upper_twist_impulse);
        apply_angular(frame, a, b, angular);
        apply_linear(frame, a, b, r_a, r_b, block.linear_impulse);
    }

    #[inline(always)]
    fn solve<W: Wide>(
        frame: &JointFrameW<W>,
        block: &mut SphericalBlock<W>,
        a: &mut BodyW<W>,
        b: &mut BodyW<W>,
        ctx: &JointSolveContext,
    ) {
        let zero = W::default();
        let quat_a = a.dq * frame.rotation_a;
        let quat_b = b.dq * frame.rotation_b;
        let relative = inverse_mul(quat_a, quat_b);

        if block.layout & SPRING != 0 {
            let c = -quat_a.rotate(delta_rotation(relative, block.target));
            let bias = c.scale(block.spring_bias);
            let cdot = b.w - a.w;
            let impulse = block
                .rotation_mass
                .mul(cdot + bias)
                .scale(-block.spring_mass_scale)
                - block.spring_impulse.scale(block.spring_impulse_scale);
            block.spring_impulse += impulse;
            apply_angular(frame, a, b, impulse);
        }

        if block.layout & MOTOR != 0 {
            let cdot = b.w - a.w;
            let lambda = -block.rotation_mass.mul(cdot - block.motor_velocity);
            let accumulated = block.motor_impulse + lambda;
            let length = accumulated.length();
            let over = length.simd_gt(block.max_motor_impulse);
            let scale = W::select(over, block.max_motor_impulse / length, W::one());
            let accumulated = accumulated.scale(scale);
            let lambda = accumulated - block.motor_impulse;
            block.motor_impulse = accumulated;
            apply_angular(frame, a, b, lambda);
        }

        if block.layout & TWIST != 0 {
            let angle = twist_angle(relative);
            let jacobian = block.twist_jacobian;
            {
                let row = limit_softness(angle - block.lower_twist, frame, ctx.inv_h, ctx.use_bias);
                let cdot = (b.w - a.w).dot(jacobian);
                let old = block.lower_twist_impulse;
                let delta = -(row.mass_scale * block.twist_mass * (cdot + row.bias))
                    - row.impulse_scale * old;
                block.lower_twist_impulse = (old + delta).max(zero);
                apply_angular(frame, a, b, jacobian.scale(block.lower_twist_impulse - old));
            }
            {
                let row = limit_softness(block.upper_twist - angle, frame, ctx.inv_h, ctx.use_bias);
                let cdot = (a.w - b.w).dot(jacobian);
                let old = block.upper_twist_impulse;
                let delta = -(row.mass_scale * block.twist_mass * (cdot + row.bias))
                    - row.impulse_scale * old;
                block.upper_twist_impulse = (old + delta).max(zero);
                apply_angular(frame, a, b, jacobian.scale(old - block.upper_twist_impulse));
            }
        }

        if block.layout & CONE != 0 {
            let angle = cone_angle(relative);
            let row = limit_softness(block.cone_angle - angle, frame, ctx.inv_h, ctx.use_bias);
            let cdot = (a.w - b.w).dot(block.swing_axis);
            let old = block.swing_impulse;
            let delta =
                -(row.mass_scale * block.swing_mass * (cdot + row.bias)) - row.impulse_scale * old;
            block.swing_impulse = (old + delta).max(zero);
            apply_angular(
                frame,
                a,
                b,
                block.swing_axis.scale(old - block.swing_impulse),
            );
        }

        let r_a = a.dq.rotate(frame.anchor_a);
        let r_b = b.dq.rotate(frame.anchor_b);
        let cdot = (b.v + b.w.cross(r_b)) - (a.v + a.w.cross(r_a));
        let (bias, mass_scale, impulse_scale) = if ctx.use_bias {
            let separation = (b.dp - a.dp) + (r_b - r_a) + frame.delta_center;
            (
                separation.scale(frame.bias_rate),
                frame.mass_scale,
                frame.impulse_scale,
            )
        } else {
            (Vec3W::zero(), W::one(), zero)
        };
        let k = point_mass(frame, r_a, r_b);
        let solved = k.solve(cdot + bias);
        let impulse = solved.scale(-mass_scale) - block.linear_impulse.scale(impulse_scale);
        block.linear_impulse += impulse;
        apply_linear(frame, a, b, r_a, r_b, impulse);
    }

    #[inline(always)]
    fn impulses<W: Wide>(block: &SphericalBlock<W>) -> [W; IMPULSE_SLOTS] {
        let mut out = [W::default(); IMPULSE_SLOTS];
        put_vec3(&mut out, LINEAR, block.linear_impulse);
        put_vec3(&mut out, SPRING_IMPULSE, block.spring_impulse);
        put_vec3(&mut out, MOTOR_IMPULSE, block.motor_impulse);
        out[LOWER_TWIST] = block.lower_twist_impulse;
        out[UPPER_TWIST] = block.upper_twist_impulse;
        out[SWING] = block.swing_impulse;
        out
    }

    fn reaction(impulses: &JointImpulses, inv_h: f32) -> (f32, f32) {
        let force = impulses.vec3(LINEAR).length() * inv_h;
        let torque = (impulses.vec3(SPRING_IMPULSE) + impulses.vec3(MOTOR_IMPULSE)).length()
            + impulses.0[SWING].abs()
            + (impulses.0[LOWER_TWIST] - impulses.0[UPPER_TWIST]).abs();
        (force, torque * inv_h)
    }
}

impl rikka_file_format::Codec for Spherical {
    type Error = crate::Error;

    fn encode(&self, out: &mut rikka_file_format::Writer<'_>) {
        out.put(&self.cone);
        out.put(&self.twist);
        out.put(&self.spring);
        out.put(&self.target);
        out.put(&self.motor);
    }

    fn decode(input: &mut rikka_file_format::Reader<'_>) -> crate::Result<Self> {
        Ok(Self {
            cone: input.get::<Option<(f32, f32)>>()?,
            twist: input.get::<Option<(f32, f32)>>()?,
            spring: input.get::<Option<Spring>>()?,
            target: input.get()?,
            motor: input.get::<Option<(Vec3, f32)>>()?,
        })
    }
}
