use rikka_physics_math::{Vec3W, Wide};

use super::math::{apply_angular, apply_linear, limit_softness, point_mass};
use super::model::{JointFrameW, JointSolveContext};
use crate::solver::lanes::BodyW;
use crate::solver::sym::SymW;

#[derive(Debug)]
pub(crate) struct Pair<'a, W: Wide> {
    pub frame: &'a JointFrameW<W>,
    pub a: &'a mut BodyW<W>,
    pub b: &'a mut BodyW<W>,
}

impl<W: Wide> Pair<'_, W> {
    #[inline(always)]
    fn apply_linear(&mut self, r_a: Vec3W<W>, r_b: Vec3W<W>, impulse: Vec3W<W>) {
        apply_linear(self.frame, self.a, self.b, r_a, r_b, impulse);
    }

    #[inline(always)]
    fn apply_angular(&mut self, impulse: Vec3W<W>) {
        apply_angular(self.frame, self.a, self.b, impulse);
    }

    #[inline(always)]
    fn point_velocity(&self, r_a: Vec3W<W>, r_b: Vec3W<W>) -> Vec3W<W> {
        (self.b.v + self.b.w.cross(r_b)) - (self.a.v + self.a.w.cross(r_a))
    }
}

#[inline(always)]
fn lock_scales<W: Wide>(frame: &JointFrameW<W>, use_bias: bool) -> (W, W, W) {
    if use_bias {
        (frame.bias_rate, frame.mass_scale, frame.impulse_scale)
    } else {
        (W::default(), W::one(), W::default())
    }
}

pub(crate) trait Jacobian<W: Wide> {
    fn velocity(&self, pair: &Pair<'_, W>) -> W;

    fn mass(&self) -> W;

    fn inverse_mass(&self) -> W;

    fn apply(&self, pair: &mut Pair<'_, W>, impulse: W);
}

#[inline(always)]
pub(crate) fn effective_mass<W: Wide>(inverse: W) -> W {
    W::select(
        inverse.simd_gt(W::default()),
        W::one() / inverse.max(W::splat(f32::MIN_POSITIVE)),
        W::default(),
    )
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RowMass<W: Wide> {
    pub inverse: W,
    pub mass: W,
}

impl<W: Wide> RowMass<W> {
    #[inline(always)]
    pub(crate) fn new(inverse: W) -> Self {
        Self {
            inverse,
            mass: effective_mass(inverse),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct AxisW<W: Wide> {
    pub axis: Vec3W<W>,
    torque_a: Vec3W<W>,
    torque_b: Vec3W<W>,
    mass: RowMass<W>,
}

impl<W: Wide> AxisW<W> {
    #[inline(always)]
    pub(crate) fn new(axis: Vec3W<W>, frame: &JointFrameW<W>) -> Self {
        let torque_a = frame.inv_i_a.mul(axis);
        let torque_b = frame.inv_i_b.mul(axis);
        Self {
            axis,
            torque_a,
            torque_b,
            mass: RowMass::new(axis.dot(torque_a + torque_b)),
        }
    }

    #[inline(always)]
    pub(crate) fn coupling(&self, other: &Self) -> W {
        self.axis.dot(other.torque_a + other.torque_b)
    }
}

impl<W: Wide> Jacobian<W> for AxisW<W> {
    #[inline(always)]
    fn velocity(&self, pair: &Pair<'_, W>) -> W {
        (pair.b.w - pair.a.w).dot(self.axis)
    }

    #[inline(always)]
    fn mass(&self) -> W {
        self.mass.mass
    }

    #[inline(always)]
    fn inverse_mass(&self) -> W {
        self.mass.inverse
    }

    #[inline(always)]
    fn apply(&self, pair: &mut Pair<'_, W>, impulse: W) {
        pair.a.w = self.torque_a.mul_add(-impulse, pair.a.w);
        pair.b.w = self.torque_b.mul_add(impulse, pair.b.w);
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LinearJacobian<W: Wide> {
    axis: Vec3W<W>,
    arm_a: Vec3W<W>,
    arm_b: Vec3W<W>,
    torque_a: Vec3W<W>,
    torque_b: Vec3W<W>,
    inverse_mass: W,
    mass: W,
}

impl<W: Wide> LinearJacobian<W> {
    #[inline(always)]
    pub(crate) fn new(
        axis: Vec3W<W>,
        lever_a: Vec3W<W>,
        lever_b: Vec3W<W>,
        frame: &JointFrameW<W>,
    ) -> Self {
        let arm_a = lever_a.cross(axis);
        let arm_b = lever_b.cross(axis);
        let torque_a = frame.inv_i_a.mul(arm_a);
        let torque_b = frame.inv_i_b.mul(arm_b);
        let inverse_mass =
            frame.inv_mass_a + frame.inv_mass_b + arm_a.dot(torque_a) + arm_b.dot(torque_b);
        Self {
            axis,
            arm_a,
            arm_b,
            torque_a,
            torque_b,
            inverse_mass,
            mass: effective_mass(inverse_mass),
        }
    }
}

impl<W: Wide> Jacobian<W> for LinearJacobian<W> {
    #[inline(always)]
    fn velocity(&self, pair: &Pair<'_, W>) -> W {
        (pair.b.v - pair.a.v).dot(self.axis) + pair.b.w.dot(self.arm_b) - pair.a.w.dot(self.arm_a)
    }

    #[inline(always)]
    fn mass(&self) -> W {
        self.mass
    }

    #[inline(always)]
    fn inverse_mass(&self) -> W {
        self.inverse_mass
    }

    #[inline(always)]
    fn apply(&self, pair: &mut Pair<'_, W>, impulse: W) {
        let frame = pair.frame;
        let linear = self.axis.scale(impulse);
        pair.a.v = linear.mul_add(-frame.inv_mass_a, pair.a.v);
        pair.a.w = self.torque_a.mul_add(-impulse, pair.a.w);
        pair.b.v = linear.mul_add(frame.inv_mass_b, pair.b.v);
        pair.b.w = self.torque_b.mul_add(impulse, pair.b.w);
    }
}

#[inline(always)]
fn clamp_length<W: Wide>(v: Vec3W<W>, max: W) -> Vec3W<W> {
    let length = v.length();
    v.scale(W::select(
        length.simd_gt(max),
        max / length.max(W::splat(f32::MIN_POSITIVE)),
        W::one(),
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LimitSides {
    pub lower: bool,
    pub upper: bool,
}

impl LimitSides {
    pub(crate) const NONE: Self = Self {
        lower: false,
        upper: false,
    };

    #[inline(always)]
    pub(crate) fn ranged(ranged: bool) -> Self {
        Self {
            lower: true,
            upper: ranged,
        }
    }

    #[inline(always)]
    pub(crate) fn any(self) -> bool {
        self.lower || self.upper
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct LimitRowW<W: Wide> {
    pub lower: W,
    pub upper: W,
    pub bilateral: W,
    pub one_sided: W,
    pub lower_impulse: W,
    pub upper_impulse: W,
}

impl<W: Wide> LimitRowW<W> {
    #[inline(always)]
    pub(crate) fn warm_start<J: Jacobian<W>>(&self, jacobian: &J, pair: &mut Pair<'_, W>) {
        jacobian.apply(pair, self.lower_impulse - self.upper_impulse);
    }

    #[inline(always)]
    pub(crate) fn solve<J: Jacobian<W>>(
        &mut self,
        jacobian: &J,
        position: W,
        ranged: bool,
        pair: &mut Pair<'_, W>,
        ctx: &JointSolveContext,
    ) {
        let cdot = jacobian.velocity(pair);
        let sides = LimitSides::ranged(ranged);
        let delta = self.delta(jacobian, cdot, position, sides, pair.frame, ctx);
        jacobian.apply(pair, delta);
    }

    #[inline(always)]
    pub(crate) fn delta<J: Jacobian<W>>(
        &mut self,
        jacobian: &J,
        cdot: W,
        position: W,
        sides: LimitSides,
        frame: &JointFrameW<W>,
        ctx: &JointSolveContext,
    ) -> W {
        let mass = jacobian.mass();
        let mut total = W::default();
        let mut cdot = cdot;
        if sides.lower {
            total = self.lower_delta(position - self.lower, cdot, mass, frame, ctx);
            cdot = total.mul_add(jacobian.inverse_mass(), cdot);
        }
        if sides.upper {
            total -= self.upper_delta(self.upper - position, -cdot, mass, frame, ctx);
        }
        total
    }

    #[inline(always)]
    pub(crate) fn solve_lower<J: Jacobian<W>>(
        &mut self,
        jacobian: &J,
        c: W,
        pair: &mut Pair<'_, W>,
        ctx: &JointSolveContext,
    ) {
        let cdot = jacobian.velocity(pair);
        let delta = self.lower_delta(c, cdot, jacobian.mass(), pair.frame, ctx);
        jacobian.apply(pair, delta);
    }

    #[inline(always)]
    fn lower_delta(
        &mut self,
        c: W,
        cdot: W,
        mass: W,
        frame: &JointFrameW<W>,
        ctx: &JointSolveContext,
    ) -> W {
        let zero = W::default();
        let bilateral = self.bilateral.simd_gt(zero);
        let limit = limit_softness(c, frame, ctx.inv_h, ctx.use_bias);
        let (bias_rate, lock_mass, lock_impulse) = lock_scales(frame, ctx.use_bias);
        let bias = W::select(bilateral, bias_rate * c, limit.bias);
        let mass_scale = W::select(bilateral, lock_mass, limit.mass_scale);
        let impulse_scale = W::select(bilateral, lock_impulse, limit.impulse_scale);
        let row_mass = mass * (self.bilateral + self.one_sided);
        let old = self.lower_impulse;
        let scaled_old = impulse_scale.mul_add(-old, old);
        let floor = W::select(bilateral, W::splat(-f32::MAX), zero);
        self.lower_impulse = (-(mass_scale * row_mass))
            .mul_add(cdot + bias, scaled_old)
            .max(floor);
        self.lower_impulse - old
    }

    #[inline(always)]
    fn upper_delta(
        &mut self,
        c: W,
        cdot: W,
        mass: W,
        frame: &JointFrameW<W>,
        ctx: &JointSolveContext,
    ) -> W {
        let row = limit_softness(c, frame, ctx.inv_h, ctx.use_bias);
        let row_mass = mass * self.one_sided;
        let old = self.upper_impulse;
        let scaled_old = row.impulse_scale.mul_add(-old, old);
        self.upper_impulse = (-(row.mass_scale * row_mass))
            .mul_add(cdot + row.bias, scaled_old)
            .max(W::default());
        self.upper_impulse - old
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DriveRowW<W: Wide> {
    pub target: W,
    pub velocity: W,
    pub bias_rate: W,
    pub mass_scale: W,
    pub impulse_scale: W,
    pub max_impulse: W,
    pub impulse: W,
}

impl<W: Wide> DriveRowW<W> {
    #[inline(always)]
    pub(crate) fn solve<J: Jacobian<W>>(&mut self, jacobian: &J, error: W, pair: &mut Pair<'_, W>) {
        let delta = self.delta(jacobian.velocity(pair), jacobian.mass(), error, true);
        jacobian.apply(pair, delta);
    }

    #[inline(always)]
    pub(crate) fn delta(&mut self, velocity: W, mass: W, error: W, capped: bool) -> W {
        let old = self.impulse;
        let offset = self.bias_rate.mul_add(error, -self.velocity);
        let scale = -(self.mass_scale * mass);
        let delta = scale.mul_add(velocity + offset, -(self.impulse_scale * old));
        if !capped {
            self.impulse = old + delta;
            return delta;
        }
        self.impulse = (old + delta).clamp(-self.max_impulse, self.max_impulse);
        self.impulse - old
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct MotorRowW<W: Wide> {
    pub velocity: W,
    pub max_impulse: W,
    pub impulse: W,
}

impl<W: Wide> MotorRowW<W> {
    #[inline(always)]
    pub(crate) fn delta(&mut self, velocity: W, mass: W) -> W {
        let cdot = velocity - self.velocity;
        let old = self.impulse;
        self.impulse = (old - mass * cdot).clamp(-self.max_impulse, self.max_impulse);
        self.impulse - old
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BlockDriveW<W: Wide> {
    pub bias_rate: W,
    pub mass_scale: W,
    pub impulse_scale: W,
    pub max_impulse: W,
    pub impulse: Vec3W<W>,
}

impl<W: Wide> BlockDriveW<W> {
    #[inline(always)]
    pub(crate) fn solve_linear(
        &mut self,
        pair: &mut Pair<'_, W>,
        lever: (Vec3W<W>, Vec3W<W>),
        error: Vec3W<W>,
        velocity: Vec3W<W>,
    ) {
        let (r_a, r_b) = lever;
        let cdot = pair.point_velocity(r_a, r_b) - velocity;
        let lambda = point_mass(pair.frame, r_a, r_b)
            .solve(error.mul_add(self.bias_rate, cdot))
            .scale(-self.mass_scale)
            - self.impulse.scale(self.impulse_scale);
        let accumulated = clamp_length(self.impulse + lambda, self.max_impulse);
        pair.apply_linear(r_a, r_b, accumulated - self.impulse);
        self.impulse = accumulated;
    }

    #[inline(always)]
    pub(crate) fn solve_angular(
        &mut self,
        pair: &mut Pair<'_, W>,
        rotation_mass: &SymW<W>,
        error: Vec3W<W>,
        velocity: Vec3W<W>,
    ) {
        let cdot = pair.b.w - pair.a.w - velocity;
        let lambda = rotation_mass
            .mul(error.mul_add(self.bias_rate, cdot))
            .scale(-self.mass_scale)
            - self.impulse.scale(self.impulse_scale);
        let accumulated = clamp_length(self.impulse + lambda, self.max_impulse);
        pair.apply_angular(accumulated - self.impulse);
        self.impulse = accumulated;
    }

    #[inline(always)]
    pub(crate) fn warm_start_linear(&self, pair: &mut Pair<'_, W>, r_a: Vec3W<W>, r_b: Vec3W<W>) {
        pair.apply_linear(r_a, r_b, self.impulse);
    }

    #[inline(always)]
    pub(crate) fn warm_start_angular(&self, pair: &mut Pair<'_, W>) {
        pair.apply_angular(self.impulse);
    }
}

#[inline(always)]
pub(crate) fn solve_point_lock<W: Wide>(
    pair: &mut Pair<'_, W>,
    lever: (Vec3W<W>, Vec3W<W>),
    error: Vec3W<W>,
    impulse: &mut Vec3W<W>,
    use_bias: bool,
) {
    let (r_a, r_b) = lever;
    let (bias_rate, mass_scale, impulse_scale) = lock_scales(pair.frame, use_bias);
    let cdot = pair.point_velocity(r_a, r_b);
    let lambda = point_mass(pair.frame, r_a, r_b)
        .solve(error.mul_add(bias_rate, cdot))
        .scale(-mass_scale)
        - impulse.scale(impulse_scale);
    *impulse += lambda;
    pair.apply_linear(r_a, r_b, lambda);
}

#[inline(always)]
pub(crate) fn solve_rotation_lock<W: Wide>(
    pair: &mut Pair<'_, W>,
    rotation_mass: &SymW<W>,
    error: Vec3W<W>,
    impulse: &mut Vec3W<W>,
    use_bias: bool,
) {
    let (bias_rate, mass_scale, impulse_scale) = lock_scales(pair.frame, use_bias);
    let cdot = pair.b.w - pair.a.w;
    let lambda = rotation_mass
        .mul(error.mul_add(bias_rate, cdot))
        .scale(-mass_scale)
        - impulse.scale(impulse_scale);
    *impulse += lambda;
    pair.apply_angular(lambda);
}

#[inline(always)]
pub(crate) fn apply_point<W: Wide>(
    pair: &mut Pair<'_, W>,
    r_a: Vec3W<W>,
    r_b: Vec3W<W>,
    impulse: Vec3W<W>,
) {
    pair.apply_linear(r_a, r_b, impulse);
}

#[inline(always)]
pub(crate) fn apply_rotation<W: Wide>(pair: &mut Pair<'_, W>, impulse: Vec3W<W>) {
    pair.apply_angular(impulse);
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SwingLockW<W: Wide> {
    xx: W,
    xy: W,
    yy: W,
}

impl<W: Wide> SwingLockW<W> {
    #[inline(always)]
    pub(crate) fn new(x: &AxisW<W>, y: &AxisW<W>, active: [W; 2]) -> Self {
        let one = W::one();
        let [active_x, active_y] = active;
        let kxx = active_x * x.inverse_mass() + (one - active_x);
        let kyy = active_y * y.inverse_mass() + (one - active_y);
        let kxy = active_x * active_y * x.coupling(y);
        let det = kxx * kyy - kxy * kxy;
        let inverse = W::select(
            det.simd_gt(W::splat(f32::MIN_POSITIVE * 1000.0)),
            one / det.max(W::splat(f32::MIN_POSITIVE)),
            W::default(),
        );
        Self {
            xx: kyy * inverse * active_x,
            xy: -kxy * inverse,
            yy: kxx * inverse * active_y,
        }
    }

    #[inline(always)]
    pub(crate) fn solve(
        &self,
        pair: &mut Pair<'_, W>,
        axes: [&AxisW<W>; 2],
        errors: [W; 2],
        impulse: &mut [W; 2],
        use_bias: bool,
    ) {
        let (bias_rate, mass_scale, impulse_scale) = lock_scales(pair.frame, use_bias);
        let [x, y] = axes;
        let relative = pair.b.w - pair.a.w;
        let rx = bias_rate.mul_add(errors[0], relative.dot(x.axis));
        let ry = bias_rate.mul_add(errors[1], relative.dot(y.axis));
        let solved_x = self.xx.mul_add(rx, self.xy * ry);
        let solved_y = self.yy.mul_add(ry, self.xy * rx);
        let delta_x = -(mass_scale * solved_x) - impulse_scale * impulse[0];
        let delta_y = -(mass_scale * solved_y) - impulse_scale * impulse[1];
        impulse[0] += delta_x;
        impulse[1] += delta_y;
        x.apply(pair, delta_x);
        y.apply(pair, delta_y);
    }
}
