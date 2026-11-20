use std::ops::RangeInclusive;

use rikka_physics_math::Wide;

use super::builder::Joint;
use super::impulses::{IMPULSE_SLOTS, JointImpulses};
use super::kinds::JointParams;
use super::model::{JointFrameW, JointModel, JointPrepareContext, JointSolveContext};
use super::rows::{DriveRowW, Jacobian, LimitRowW, LinearJacobian, Pair};
use super::spring::Spring;
use crate::BodyId;
use crate::solver::lanes::{BodyW, MAX_LANES};
use crate::solver::softness::Softness;

const RIGID: u32 = 1;
const SPRING: u32 = 1 << 1;
const LIMIT: u32 = 1 << 2;
const MOTOR: u32 = 1 << 3;

const SPRING_IMPULSE: usize = 0;
const LOWER_IMPULSE: usize = 1;
const UPPER_IMPULSE: usize = 2;
const MOTOR_IMPULSE: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Distance {
    length: f32,
    range: Option<(f32, f32)>,
    spring: Option<Spring>,
    spring_force: (f32, f32),
    motor: Option<(f32, f32)>,
}

impl Distance {
    pub fn new(body_a: BodyId, body_b: BodyId) -> Joint<Distance> {
        Joint::new(
            body_a,
            body_b,
            Distance {
                length: 1.0,
                range: None,
                spring: None,
                spring_force: (-f32::MAX, f32::MAX),
                motor: None,
            },
        )
    }

    pub fn length(&self) -> f32 {
        self.length
    }

    pub fn range(&self) -> Option<(f32, f32)> {
        self.range
    }

    pub fn spring(&self) -> Option<Spring> {
        self.spring
    }

    pub fn spring_force(&self) -> (f32, f32) {
        self.spring_force
    }

    pub fn motor(&self) -> Option<(f32, f32)> {
        self.motor
    }

    pub fn set_length(&mut self, length: f32) {
        self.length = length;
    }

    pub fn set_motor_speed(&mut self, speed: f32) {
        if let Some((motor_speed, _)) = &mut self.motor {
            *motor_speed = speed;
        }
    }

    pub(crate) fn is_valid(&self) -> bool {
        let range = self.range.is_none_or(|(min, max)| {
            min.is_finite() && max.is_finite() && 0.0 <= min && min <= max
        });
        let (lower, upper) = self.spring_force;
        let motor = self
            .motor
            .is_none_or(|(speed, force)| speed.is_finite() && force >= 0.0);
        self.length.is_finite()
            && self.length >= 0.0
            && range
            && lower <= upper
            && motor
            && self.spring.is_none_or(|spring| spring.is_valid())
    }
}

impl Joint<Distance> {
    pub fn length(mut self, length: f32) -> Self {
        self.params.length = length;
        self
    }

    pub fn range(mut self, lengths: RangeInclusive<f32>) -> Self {
        self.params.range = Some((*lengths.start(), *lengths.end()));
        self
    }

    pub fn spring(mut self, spring: Spring) -> Self {
        self.params.spring = Some(spring);
        self
    }

    pub fn spring_force(mut self, forces: RangeInclusive<f32>) -> Self {
        self.params.spring_force = (*forces.start(), *forces.end());
        self
    }

    pub fn motor(mut self, speed: f32, max_force: f32) -> Self {
        self.params.motor = Some((speed, max_force));
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DistanceBlock<W: Wide> {
    limit: LimitRowW<W>,
    motor: DriveRowW<W>,
    length: W,
    spring_bias: W,
    spring_mass_scale: W,
    spring_impulse_scale: W,
    lower_spring_impulse: W,
    upper_spring_impulse: W,
    spring_impulse: W,
    layout: u32,
}

#[derive(Clone, Copy, Debug)]
struct Axial<W: Wide> {
    length: W,
    jacobian: LinearJacobian<W>,
}

impl<W: Wide> Axial<W> {
    #[inline(always)]
    fn new(frame: &JointFrameW<W>, a: &BodyW<W>, b: &BodyW<W>) -> Self {
        let r_a = a.dq.rotate(frame.anchor_a);
        let r_b = b.dq.rotate(frame.anchor_b);
        let separation = (b.dp - a.dp) + (r_b - r_a) + frame.delta_center;
        Self {
            length: separation.length(),
            jacobian: LinearJacobian::new(separation.normalize_or_zero(), r_a, r_b, frame),
        }
    }
}

#[derive(Debug)]
pub(crate) struct DistanceModel;

impl JointModel for DistanceModel {
    type Params = Distance;
    type Block<W: Wide> = DistanceBlock<W>;

    fn params(all: &JointParams) -> &Distance {
        match all {
            JointParams::Distance(params) => params,
            _ => unreachable!("joint batch mixes kinds"),
        }
    }

    fn layout(params: &Distance) -> u32 {
        if params.spring.is_none() && params.range.is_none() && params.motor.is_none() {
            return RIGID;
        }
        (params.spring.is_some() as u32 * SPRING)
            | (params.range.is_some() as u32 * LIMIT)
            | (params.motor.is_some() as u32 * MOTOR)
    }

    #[inline(always)]
    fn prepare<W: Wide>(
        block: &mut DistanceBlock<W>,
        frame: &JointFrameW<W>,
        params: &[&Distance; MAX_LANES],
        impulses: &[&JointImpulses; MAX_LANES],
        ctx: &JointPrepareContext,
    ) {
        let layout = Self::layout(params[0]);
        let warm = |slot: usize| W::from_fn(|lane| impulses[lane].0[slot] * ctx.warm_start);
        let separation = frame.delta_center + frame.anchor_b - frame.anchor_a;
        let mass = LinearJacobian::new(
            separation.normalize_or_zero(),
            frame.anchor_a,
            frame.anchor_b,
            frame,
        )
        .mass();
        let soft: [Softness; MAX_LANES] = std::array::from_fn(|lane| match params[lane].spring {
            Some(spring) if lane < W::LANES => {
                spring.softness(mass.lane(lane), ctx.h, ctx.max_spring_hertz)
            }
            _ => Softness::default(),
        });
        let rigid = layout & RIGID != 0;
        let bounds = |lane: usize| {
            let distance = &params[lane];
            if rigid {
                (distance.length, distance.length)
            } else {
                distance.range.unwrap_or((0.0, 0.0))
            }
        };
        let flag = |on: bool| if on { 1.0 } else { 0.0 };
        *block = DistanceBlock {
            limit: LimitRowW {
                lower: W::from_fn(|lane| bounds(lane).0),
                upper: W::from_fn(|lane| bounds(lane).1),
                bilateral: W::splat(flag(rigid)),
                one_sided: W::splat(flag(layout & LIMIT != 0)),
                lower_impulse: warm(LOWER_IMPULSE),
                upper_impulse: warm(UPPER_IMPULSE),
            },
            motor: DriveRowW {
                target: W::default(),
                velocity: W::from_fn(|lane| params[lane].motor.map_or(0.0, |motor| motor.0)),
                bias_rate: W::default(),
                mass_scale: W::one(),
                impulse_scale: W::default(),
                max_impulse: W::from_fn(|lane| {
                    params[lane].motor.map_or(0.0, |motor| motor.1 * ctx.h)
                }),
                impulse: warm(MOTOR_IMPULSE),
            },
            length: W::from_fn(|lane| params[lane].length),
            spring_bias: W::from_fn(|lane| soft[lane].bias_rate),
            spring_mass_scale: W::from_fn(|lane| soft[lane].mass_scale),
            spring_impulse_scale: W::from_fn(|lane| soft[lane].impulse_scale),
            lower_spring_impulse: W::from_fn(|lane| params[lane].spring_force.0 * ctx.h),
            upper_spring_impulse: W::from_fn(|lane| params[lane].spring_force.1 * ctx.h),
            spring_impulse: warm(SPRING_IMPULSE),
            layout,
        };
    }

    #[inline(always)]
    fn warm_start<W: Wide>(
        frame: &JointFrameW<W>,
        block: &DistanceBlock<W>,
        a: &mut BodyW<W>,
        b: &mut BodyW<W>,
    ) {
        let axial = Axial::new(frame, a, b);
        let total = block.spring_impulse + block.limit.lower_impulse - block.limit.upper_impulse
            + block.motor.impulse;
        axial.jacobian.apply(&mut Pair { frame, a, b }, total);
    }

    #[inline(always)]
    fn solve<W: Wide>(
        frame: &JointFrameW<W>,
        block: &mut DistanceBlock<W>,
        a: &mut BodyW<W>,
        b: &mut BodyW<W>,
        ctx: &JointSolveContext,
    ) {
        let axial = Axial::new(frame, a, b);
        let jacobian = axial.jacobian;
        let mut pair = Pair { frame, a, b };
        if block.layout & SPRING != 0 {
            let mass = jacobian.mass();
            let c = axial.length - block.length;
            let cdot = jacobian.velocity(&pair);
            let old = block.spring_impulse;
            let delta = -(block.spring_mass_scale * mass * block.spring_bias.mul_add(c, cdot))
                - block.spring_impulse_scale * old;
            block.spring_impulse =
                (old + delta).clamp(block.lower_spring_impulse, block.upper_spring_impulse);
            jacobian.apply(&mut pair, block.spring_impulse - old);
        }
        if block.layout & (RIGID | LIMIT) != 0 {
            block.limit.solve(
                &jacobian,
                axial.length,
                block.layout & LIMIT != 0,
                &mut pair,
                ctx,
            );
        }
        if block.layout & MOTOR != 0 {
            block.motor.solve(&jacobian, W::default(), &mut pair);
        }
    }

    #[inline(always)]
    fn impulses<W: Wide>(block: &DistanceBlock<W>) -> [W; IMPULSE_SLOTS] {
        let mut out = [W::default(); IMPULSE_SLOTS];
        out[SPRING_IMPULSE] = block.spring_impulse;
        out[LOWER_IMPULSE] = block.limit.lower_impulse;
        out[UPPER_IMPULSE] = block.limit.upper_impulse;
        out[MOTOR_IMPULSE] = block.motor.impulse;
        out
    }

    fn reaction(impulses: &JointImpulses, inv_h: f32) -> (f32, f32) {
        let axial = impulses.0[SPRING_IMPULSE] + impulses.0[LOWER_IMPULSE]
            - impulses.0[UPPER_IMPULSE]
            + impulses.0[MOTOR_IMPULSE];
        (axial.abs() * inv_h, 0.0)
    }
}

impl rikka_file_format::Codec for Distance {
    type Error = crate::Error;

    fn encode(&self, out: &mut rikka_file_format::Writer<'_>) {
        out.put(&self.length);
        out.put(&self.range);
        out.put(&self.spring);
        out.put(&self.spring_force);
        out.put(&self.motor);
    }

    fn decode(input: &mut rikka_file_format::Reader<'_>) -> crate::Result<Self> {
        Ok(Self {
            length: input.get::<f32>()?,
            range: input.get::<Option<(f32, f32)>>()?,
            spring: input.get::<Option<Spring>>()?,
            spring_force: input.get()?,
            motor: input.get::<Option<(f32, f32)>>()?,
        })
    }
}
