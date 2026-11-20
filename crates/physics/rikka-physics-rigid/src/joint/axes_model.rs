use std::f32::consts::{PI, TAU};

use rikka_physics_math::{Mask, MaskW, QuatW, Vec3, Vec3W, Wide};

use super::axes::{
    ANGULAR_BLOCK, ANGULAR_DRIVES, ANGULAR_MOTORS, ANGULAR_SPRINGS, Axes, CONE, LINEAR_BLOCK,
    LINEAR_DRIVES, LINEAR_MOTORS, LINEAR_RANGES, LINEAR_ROWS, POSE_ANGULAR, POSE_LINEAR, SWING,
    SWING_RANGES, TWIST, TWIST_RANGE,
};
use super::axes_params::AxesParams;
use super::columns::{columns, columns_from};
use super::drive_table::{ANGULAR_POSE, DriveLanes, DriveSettingsW, LINEAR_POSE};
use super::geometry::{JointGeometryW, relative_rotation};
use super::impulses::{IMPULSE_SLOTS, JointImpulses, put_vec3};
use super::kinds::JointParams;
use super::math::axis;
use super::model::{JointFrameW, JointModel, JointPrepareContext, JointSolveContext};
use super::rows::{
    AxisW, BlockDriveW, DriveRowW, Jacobian, LimitRowW, LimitSides, LinearJacobian, MotorRowW,
    Pair, SwingLockW, apply_point, apply_rotation, effective_mass, solve_point_lock,
    solve_rotation_lock,
};
use super::swing_twist::{
    cone, cone_angle, elliptic_limit, swing_angles, swing_twist, twist_angle,
};
use crate::solver::lanes::{BodyW, MAX_LANES};
use crate::solver::sym::SymW;

const LIMIT_LOWER: usize = 0;
const LIMIT_UPPER: usize = 6;
const DRIVE: usize = 12;
const POSE_LINEAR_IMPULSE: usize = 18;
const POSE_ANGULAR_IMPULSE: usize = 21;
const MOTOR: usize = 24;
const LINEAR_JACOBIANS: u32 = LINEAR_ROWS | LINEAR_DRIVES | LINEAR_MOTORS;
const ANGULAR_ROWS: u32 = TWIST | SWING | ANGULAR_DRIVES | ANGULAR_MOTORS;
const DRIVES: u32 = LINEAR_DRIVES | ANGULAR_DRIVES;
const MOTORS: u32 = LINEAR_MOTORS | ANGULAR_MOTORS;
const POSES: u32 = POSE_LINEAR | POSE_ANGULAR;
const LOWER_SWING_X: u32 = 1;
const LOWER_SWING_Y: u32 = 1 << 1;
const LOWER_TWIST: u32 = 1 << 2;
const ACTIVE_CONE: u32 = 1 << 3;
const SHIFTED_LOCK: u32 = 1 << 4;
const UPPER_SWING_X: u32 = 1 << 5;
const UPPER_SWING_Y: u32 = 1 << 6;
const UPPER_TWIST: u32 = 1 << 7;
const CAPPED_DRIVES: u32 = 1 << 8;
const LOWER_LIMITS: [u32; 3] = [LOWER_SWING_X, LOWER_SWING_Y, LOWER_TWIST];
const UPPER_LIMITS: [u32; 3] = [UPPER_SWING_X, UPPER_SWING_Y, UPPER_TWIST];
const SWING_LIMITS: u32 = LOWER_SWING_X | LOWER_SWING_Y | UPPER_SWING_X | UPPER_SWING_Y;
const TWIST_LIMITS: u32 = LOWER_TWIST | UPPER_TWIST;
const ANGLE_LIMITS: u32 = SWING_LIMITS | TWIST_LIMITS | ACTIVE_CONE;

#[derive(Clone, Copy, Debug)]
pub(crate) struct AxesBlock<W: Wide> {
    angular: [AxisW<W>; 3],
    couplings: [W; 3],
    limits: [LimitRowW<W>; 6],
    drives: [DriveRowW<W>; 6],
    motors: [MotorRowW<W>; 6],
    cone: AxisW<W>,
    cone_limit: W,
    twist_margin: W,
    swing_margin: W,
    swing_lock: SwingLockW<W>,
    rotation_mass: SymW<W>,
    linear_pose: BlockDriveW<W>,
    angular_pose: BlockDriveW<W>,
    locked_rotation: QuatW<W>,
    target_position: Vec3W<W>,
    target_rotation: QuatW<W>,
    linear_velocity: Vec3W<W>,
    angular_velocity: Vec3W<W>,
    layout: u32,
    active: u32,
}

impl<W: Wide> AxesBlock<W> {
    #[inline(always)]
    fn far_from_limits(&self, relative: QuatW<W>, spin: W, ctx: &JointSolveContext) -> bool {
        let [_, _, _, x, y, twist] = &self.limits;
        let pushed = x.lower_impulse
            + x.upper_impulse
            + y.lower_impulse
            + y.upper_impulse
            + twist.lower_impulse
            + twist.upper_impulse;
        let zero = W::default();
        let travel = spin * W::splat(2.0 / ctx.inv_h);
        let q = relative;
        let twist_room = self.twist_margin - travel;
        let swing_room = self.swing_margin - travel;
        let twist_far =
            (W::splat(2.0) * q.v.z.abs()).simd_lt(twist_room * q.s) & twist_room.simd_gt(zero);
        let swing = q.v.x * q.v.x + q.v.y * q.v.y;
        let upright = q.v.z * q.v.z + q.s * q.s;
        let swing_far = (W::splat(4.0) * swing).simd_lt(swing_room * swing_room * upright)
            & swing_room.simd_gt(zero);
        let far = twist_far & swing_far & pushed.simd_eq(zero);
        far.bits() == (1 << W::LANES) - 1
    }

    #[inline(always)]
    fn locked_offset(&self) -> Vec3W<W> {
        Vec3W::new(
            self.limits[0].lower,
            self.limits[1].lower,
            self.limits[2].lower,
        )
    }

    #[inline(always)]
    fn block_impulse(&self, first: usize) -> Vec3W<W> {
        Vec3W::new(
            self.limits[first].lower_impulse,
            self.limits[first + 1].lower_impulse,
            self.limits[first + 2].lower_impulse,
        )
    }

    #[inline(always)]
    fn set_block_impulse(&mut self, first: usize, impulse: Vec3W<W>) {
        self.limits[first].lower_impulse = impulse.x;
        self.limits[first + 1].lower_impulse = impulse.y;
        self.limits[first + 2].lower_impulse = impulse.z;
    }

    #[inline(always)]
    fn limit_sides(&self, index: usize) -> LimitSides {
        if index < 2 && self.layout & SWING_RANGES == 0 {
            return LimitSides::NONE;
        }
        LimitSides {
            lower: self.active & LOWER_LIMITS[index] != 0,
            upper: self.active & UPPER_LIMITS[index] != 0,
        }
    }

    #[inline(always)]
    fn angular_limit(&self, index: usize) -> bool {
        self.limit_sides(index).any()
    }

    #[inline(always)]
    fn lock_lever(&self, geometry: &JointGeometryW<W>) -> ((Vec3W<W>, Vec3W<W>), Vec3W<W>) {
        if self.active & SHIFTED_LOCK != 0 {
            pose_lever(geometry, self.locked_offset())
        } else {
            ((geometry.r_a, geometry.r_b), geometry.separation)
        }
    }

    #[inline(always)]
    fn angular_delta(
        &mut self,
        index: usize,
        cdot: W,
        angle: W,
        sides: LimitSides,
        frame: &JointFrameW<W>,
        ctx: &JointSolveContext,
    ) -> W {
        let layout = self.layout;
        let capped = self.active & CAPPED_DRIVES != 0;
        let axis = &self.angular[index];
        let mut cdot = cdot;
        let mut total = W::default();
        if layout & ANGULAR_DRIVES != 0 {
            let drive = &mut self.drives[3 + index];
            let delta = drive.delta(cdot, axis.mass(), wrap(angle - drive.target), capped);
            cdot = delta.mul_add(axis.inverse_mass(), cdot);
            total += delta;
        }
        if layout & ANGULAR_MOTORS != 0 {
            let delta = self.motors[3 + index].delta(cdot, axis.mass());
            cdot = delta.mul_add(axis.inverse_mass(), cdot);
            total += delta;
        }
        if sides.any() {
            total += self.limits[3 + index].delta(axis, cdot, angle, sides, frame, ctx);
        }
        total
    }

    #[inline(always)]
    fn solve_angular_rows(
        &mut self,
        angles: [W; 3],
        sides: [LimitSides; 3],
        pair: &mut Pair<'_, W>,
        ctx: &JointSolveContext,
    ) {
        let drives = self.layout & (ANGULAR_DRIVES | ANGULAR_MOTORS) != 0;
        let relative = pair.b.w - pair.a.w;
        let mut cdots = [
            relative.dot(self.angular[0].axis),
            relative.dot(self.angular[1].axis),
            relative.dot(self.angular[2].axis),
        ];
        let mut totals = [W::default(); 3];
        for index in 0..3 {
            if !drives && !sides[index].any() {
                continue;
            }
            let total = self.angular_delta(
                index,
                cdots[index],
                angles[index],
                sides[index],
                pair.frame,
                ctx,
            );
            for (later, cdot) in cdots.iter_mut().enumerate().skip(index + 1) {
                *cdot = total.mul_add(self.couplings[index + later - 1], *cdot);
            }
            totals[index] = total;
        }
        for (index, total) in totals.into_iter().enumerate() {
            if drives || sides[index].any() {
                self.angular[index].apply(pair, total);
            }
        }
    }

    #[inline(always)]
    fn solve_linear_axis(
        &mut self,
        linear: &LinearRows<W>,
        index: usize,
        limit: bool,
        pair: &mut Pair<'_, W>,
        ctx: &JointSolveContext,
    ) {
        let layout = self.layout;
        let jacobian = &linear.jacobians[index];
        let position = linear.positions[index];
        let mut cdot = jacobian.velocity(pair);
        let mut total = W::default();
        if layout & LINEAR_DRIVES != 0 {
            let drive = &mut self.drives[index];
            let capped = self.active & CAPPED_DRIVES != 0;
            let delta = drive.delta(cdot, jacobian.mass(), position - drive.target, capped);
            cdot = delta.mul_add(jacobian.inverse_mass(), cdot);
            total += delta;
        }
        if layout & LINEAR_MOTORS != 0 {
            let delta = self.motors[index].delta(cdot, jacobian.mass());
            cdot = delta.mul_add(jacobian.inverse_mass(), cdot);
            total += delta;
        }
        if limit {
            let sides = LimitSides::ranged(layout & LINEAR_RANGES != 0);
            total += self.limits[index].delta(jacobian, cdot, position, sides, pair.frame, ctx);
        }
        jacobian.apply(pair, total);
    }

    #[inline(always)]
    fn warm_impulse(&self, index: usize, drives: u32, motors: u32, limit: bool) -> W {
        let mut total = W::default();
        if self.layout & drives != 0 {
            total += self.drives[index].impulse;
        }
        if self.layout & motors != 0 {
            total += self.motors[index].impulse;
        }
        if limit {
            total += self.limits[index].lower_impulse - self.limits[index].upper_impulse;
        }
        total
    }
}

#[derive(Clone, Copy, Debug)]
struct LinearRows<W: Wide> {
    jacobians: [LinearJacobian<W>; 3],
    positions: [W; 3],
}

impl<W: Wide> LinearRows<W> {
    #[inline(always)]
    fn new(geometry: &JointGeometryW<W>, frame: &JointFrameW<W>) -> Self {
        let lever_a = geometry.r_a + geometry.separation;
        let x = geometry.world_axis(Vec3W::splat(Vec3::X));
        let y = geometry.world_axis(Vec3W::splat(Vec3::Y));
        let z = geometry.world_axis(Vec3W::splat(Vec3::Z));
        let r_b = geometry.r_b;
        Self {
            jacobians: [
                LinearJacobian::new(x, lever_a, r_b, frame),
                LinearJacobian::new(y, lever_a, r_b, frame),
                LinearJacobian::new(z, lever_a, r_b, frame),
            ],
            positions: [
                x.dot(geometry.separation),
                y.dot(geometry.separation),
                z.dot(geometry.separation),
            ],
        }
    }

    #[inline(always)]
    fn when(enabled: bool, geometry: &JointGeometryW<W>, frame: &JointFrameW<W>) -> Option<Self> {
        if enabled {
            Some(Self::new(geometry, frame))
        } else {
            None
        }
    }
}

#[inline(always)]
fn pose_lever<W: Wide>(
    geometry: &JointGeometryW<W>,
    offset: Vec3W<W>,
) -> ((Vec3W<W>, Vec3W<W>), Vec3W<W>) {
    let offset = geometry.world_axis(offset);
    (
        (geometry.r_a + offset, geometry.r_b),
        geometry.separation - offset,
    )
}

#[inline(always)]
fn wrap<W: Wide>(angle: W) -> W {
    let pi = W::splat(PI);
    let tau = W::splat(TAU);
    let angle = W::select(angle.simd_gt(pi), angle - tau, angle);
    W::select(angle.simd_lt(-pi), angle + tau, angle)
}

#[inline(always)]
fn linear_mass<W: Wide>(frame: &JointFrameW<W>, unit: Vec3, lever_a: Vec3W<W>) -> W {
    LinearJacobian::new(axis(frame.rotation_a, unit), lever_a, frame.anchor_b, frame).mass()
}

#[inline(always)]
fn axis_rows<'a, const N: usize>(
    axes: &[&'a Axes; MAX_LANES],
    pick: fn(&Axes) -> &[f32; N],
) -> [&'a [f32]; MAX_LANES] {
    let mut rows: [&[f32]; MAX_LANES] = [&[]; MAX_LANES];
    for (row, axes) in rows.iter_mut().zip(axes) {
        *row = pick(axes);
    }
    rows
}

#[inline(always)]
fn block_drive<W: Wide>(settings: DriveSettingsW<W>, impulse: Vec3W<W>) -> BlockDriveW<W> {
    BlockDriveW {
        bias_rate: settings.bias_rate,
        mass_scale: settings.mass_scale,
        impulse_scale: settings.impulse_scale,
        max_impulse: settings.max_impulse,
        impulse,
    }
}

#[inline(always)]
fn fill_limits<W: Wide>(
    block: &mut AxesBlock<W>,
    axes: &[&Axes; MAX_LANES],
    impulses: &[&[f32]; MAX_LANES],
    warm: W,
) {
    let zero = W::default();
    let one = W::one();
    let lower: [W; 6] = columns(&axis_rows(axes, Axes::lower), 0);
    let upper: [W; 6] = columns(&axis_rows(axes, Axes::upper), 0);
    let stored: [W; 12] = columns(impulses, LIMIT_LOWER);
    for index in 0..6 {
        let bilateral = lower[index].simd_eq(upper[index]);
        let one_sided = lower[index].simd_lt(upper[index]);
        let bounded = bilateral | one_sided;
        block.limits[index] = LimitRowW {
            lower: lower[index].masked(bounded),
            upper: upper[index].masked(bounded),
            bilateral: one.masked(bilateral),
            one_sided: one.masked(one_sided),
            lower_impulse: stored[index] * warm,
            upper_impulse: stored[LIMIT_UPPER - LIMIT_LOWER + index] * warm,
        };
    }
    if block.layout & CONE != 0 {
        for (index, one_sided) in [(3, one), (4, zero)] {
            let row = &mut block.limits[index];
            row.lower = zero;
            row.upper = zero;
            row.bilateral = zero;
            row.one_sided = one_sided;
        }
    }
}

#[inline(always)]
fn vector<W: Wide>([x, y, z]: [W; 3]) -> Vec3W<W> {
    Vec3W::new(x, y, z)
}

#[inline(always)]
fn fill_drives<W: Wide>(
    block: &mut AxesBlock<W>,
    frame: &JointFrameW<W>,
    axes: &[&Axes; MAX_LANES],
    impulses: &[&[f32]; MAX_LANES],
    warm: W,
    ctx: &JointPrepareContext,
) -> bool {
    let layout = block.layout;
    let zero = W::default();
    let mut tables = [axes[0].drives(); MAX_LANES];
    for (table, axes) in tables.iter_mut().zip(axes) {
        *table = axes.drives();
    }
    let drives = DriveLanes::<W>::new(tables, ctx.h, ctx.max_spring_hertz);
    let targets = axis_rows(axes, Axes::targets);
    let velocities = axis_rows(axes, Axes::velocities);
    let caps = axis_rows(axes, Axes::motor_caps);
    let mut capped = false;
    for (drive_group, motor_group, pose_group, first) in [
        (LINEAR_DRIVES, LINEAR_MOTORS, POSE_LINEAR, 0),
        (ANGULAR_DRIVES, ANGULAR_MOTORS, POSE_ANGULAR, 3),
    ] {
        if layout & (drive_group | motor_group | pose_group) == 0 {
            continue;
        }
        let target: [W; 3] = columns_from(&targets, first);
        let velocity: [W; 3] = columns_from(&velocities, first);
        let cap: [W; 3] = if layout & motor_group != 0 {
            columns_from(&caps, first)
        } else {
            [zero; 3]
        };
        if layout & drive_group != 0 {
            let masses = if first == 0 {
                let lever_a = frame.delta_center + frame.anchor_b;
                [
                    linear_mass(frame, Vec3::X, lever_a),
                    linear_mass(frame, Vec3::Y, lever_a),
                    linear_mass(frame, Vec3::Z, lever_a),
                ]
            } else {
                [
                    block.angular[0].mass(),
                    block.angular[1].mass(),
                    block.angular[2].mass(),
                ]
            };
            let settings = drives.settings(first, masses);
            let stored: [W; 3] = columns_from(impulses, DRIVE + first);
            for index in 0..3 {
                let settings = settings[index];
                capped |= settings.max_impulse.simd_lt(W::splat(f32::MAX)).any();
                block.drives[first + index] = DriveRowW {
                    target: target[index],
                    velocity: velocity[index].masked(cap[index].simd_le(zero)),
                    bias_rate: settings.bias_rate,
                    mass_scale: settings.mass_scale,
                    impulse_scale: settings.impulse_scale,
                    max_impulse: settings.max_impulse,
                    impulse: stored[index] * warm,
                };
            }
        }
        if layout & motor_group != 0 {
            let stored: [W; 3] = columns_from(impulses, MOTOR + first);
            for index in 0..3 {
                block.motors[first + index] = MotorRowW {
                    velocity: velocity[index],
                    max_impulse: W::select(
                        cap[index].simd_ge(W::splat(f32::MAX)),
                        W::splat(f32::MAX),
                        cap[index] * W::splat(ctx.h),
                    ),
                    impulse: stored[index] * warm,
                };
            }
        }
        if layout & pose_group == 0 {
            continue;
        }
        if first == 0 {
            block.target_position = vector(target);
            block.linear_velocity = vector(velocity);
            let [settings] = drives.settings(
                LINEAR_POSE,
                [effective_mass(frame.inv_mass_a + frame.inv_mass_b)],
            );
            let stored = vector(columns_from(impulses, POSE_LINEAR_IMPULSE));
            block.linear_pose = block_drive(settings, stored.scale(warm));
        } else {
            let [x, y, z, s]: [W; 4] = columns(&axis_rows(axes, Axes::rotation), 0);
            block.target_rotation = QuatW::new(Vec3W::new(x, y, z), s);
            block.angular_velocity = vector(velocity);
            let inertia = frame.inv_i_a.add(&frame.inv_i_b);
            let [settings] = drives.settings(
                ANGULAR_POSE,
                [effective_mass(
                    (inertia.xx + inertia.yy + inertia.zz) * W::splat(1.0 / 3.0),
                )],
            );
            let stored = vector(columns_from(impulses, POSE_ANGULAR_IMPULSE));
            block.angular_pose = block_drive(settings, stored.scale(warm));
        }
    }
    capped
}

#[inline(always)]
fn present_sides<W: Wide>(row: &LimitRowW<W>, bits: [u32; 2]) -> u32 {
    let zero = W::default();
    let one_sided = row.one_sided.simd_gt(zero);
    let lower = row.bilateral.simd_gt(zero) | one_sided;
    (if lower.any() { bits[0] } else { 0 }) | (if one_sided.any() { bits[1] } else { 0 })
}

#[inline(always)]
fn side_margin<W: Wide>(row: &LimitRowW<W>) -> W {
    W::select(
        row.one_sided.simd_gt(W::default()),
        (-row.lower).min(row.upper),
        W::default(),
    )
}

#[inline(always)]
fn limit_margins<W: Wide>(layout: u32, limits: &[LimitRowW<W>; 6]) -> [W; 2] {
    let open = W::splat(f32::MAX);
    let twist = if layout & TWIST == 0 {
        open
    } else if layout & TWIST_RANGE == 0 {
        W::default()
    } else {
        side_margin(&limits[5])
    };
    let swing = if layout & SWING == 0 {
        open
    } else if layout & SWING_RANGES == 0 {
        W::default()
    } else {
        side_margin(&limits[3]).min(side_margin(&limits[4]))
    };
    [twist, swing]
}

#[inline(always)]
fn active_limits<W: Wide>(layout: u32, limits: &[LimitRowW<W>; 6]) -> u32 {
    let mut active = 0;
    if layout & TWIST != 0 {
        active |= if layout & TWIST_RANGE == 0 {
            LOWER_TWIST
        } else {
            present_sides(&limits[5], [LOWER_TWIST, UPPER_TWIST])
        };
    }
    if layout & SWING != 0 && layout & SWING_RANGES == 0 {
        active |= LOWER_SWING_X | LOWER_SWING_Y;
    }
    if layout & SWING_RANGES != 0 {
        active |= present_sides(&limits[3], [LOWER_SWING_X, UPPER_SWING_X]);
        active |= present_sides(&limits[4], [LOWER_SWING_Y, UPPER_SWING_Y]);
    }
    if layout & CONE != 0 {
        active |= ACTIVE_CONE;
    }
    active
}

#[derive(Debug)]
pub(crate) struct AxesModel;

impl JointModel for AxesModel {
    type Params = AxesParams;
    type Block<W: Wide> = AxesBlock<W>;

    fn params(all: &JointParams) -> &AxesParams {
        match all {
            JointParams::Axes(params) => params,
            _ => unreachable!("joint batch mixes kinds"),
        }
    }

    fn layout(params: &AxesParams) -> u32 {
        params.axes().layout()
    }

    #[inline(always)]
    fn prepare<W: Wide>(
        block: &mut AxesBlock<W>,
        frame: &JointFrameW<W>,
        params: &[&AxesParams; MAX_LANES],
        impulses: &[&JointImpulses; MAX_LANES],
        ctx: &JointPrepareContext,
    ) {
        let mut axes = [params[0].axes(); MAX_LANES];
        for (axes, params) in axes.iter_mut().zip(params) {
            *axes = params.axes();
        }
        let mut stored: [&[f32]; MAX_LANES] = [&[]; MAX_LANES];
        for (row, impulses) in stored.iter_mut().zip(impulses) {
            *row = &impulses.0;
        }
        let layout = axes[0].layout();
        let warm = W::splat(ctx.warm_start);
        block.layout = layout;
        let relative = relative_rotation(frame.rotation_a, frame.rotation_b);
        if layout & ANGULAR_ROWS != 0 {
            let decomposed = swing_twist(relative);
            let rotation_a = frame.rotation_a;
            block.angular = [
                AxisW::new(rotation_a.rotate(decomposed.swing_x), frame),
                AxisW::new(rotation_a.rotate(decomposed.swing_y), frame),
                AxisW::new(rotation_a.rotate(decomposed.twist), frame),
            ];
            let [x, y, z] = &block.angular;
            block.couplings = [x.coupling(y), x.coupling(z), y.coupling(z)];
        }
        fill_limits(block, &axes, &stored, warm);
        let capped = layout & (DRIVES | MOTORS | POSES) != 0
            && fill_drives(block, frame, &axes, &stored, warm, ctx);
        block.cone_limit = W::default();
        [block.twist_margin, block.swing_margin] = limit_margins(layout, &block.limits);
        if layout & CONE != 0 {
            let swing = cone(relative);
            let cone_x = W::from_fn(|lane| axes[lane].cone().map_or(0.0, |cone| cone.0));
            let cone_y = W::from_fn(|lane| axes[lane].cone().map_or(0.0, |cone| cone.1));
            block.cone = AxisW::new(-frame.rotation_a.rotate(swing.jacobian), frame);
            block.cone_limit = elliptic_limit(cone_x, cone_y, swing.direction_x, swing.direction_y);
            block.swing_margin = cone_x.min(cone_y);
        }
        block.active = active_limits(layout, &block.limits);
        if capped {
            block.active |= CAPPED_DRIVES;
        }
        if layout & SWING != 0 && layout & SWING_RANGES == 0 {
            block.swing_lock = SwingLockW::new(
                &block.angular[0],
                &block.angular[1],
                [block.limits[3].bilateral, block.limits[4].bilateral],
            );
        }
        if layout & LINEAR_BLOCK != 0 {
            let offset = block.locked_offset();
            let zero = W::default();
            if (!(offset.x.simd_eq(zero) & offset.y.simd_eq(zero) & offset.z.simd_eq(zero))).any() {
                block.active |= SHIFTED_LOCK;
            }
        }
        if layout & (ANGULAR_BLOCK | POSE_ANGULAR) != 0 {
            block.rotation_mass = frame.inv_i_a.add(&frame.inv_i_b).inverse();
        }
        if layout & ANGULAR_BLOCK != 0 {
            let [x, y, z, s]: [W; 4] = columns(&axis_rows(&axes, Axes::locked), 0);
            block.locked_rotation = QuatW::new(Vec3W::new(x, y, z), s);
        }
    }

    #[inline(always)]
    fn warm_start<W: Wide>(
        frame: &JointFrameW<W>,
        block: &AxesBlock<W>,
        a: &mut BodyW<W>,
        b: &mut BodyW<W>,
    ) {
        let layout = block.layout;
        let geometry = JointGeometryW::new(frame, a, b);
        let mut pair = Pair { frame, a, b };
        for index in 0..3 {
            let limit = block.active & (LOWER_LIMITS[index] | UPPER_LIMITS[index]) != 0;
            if layout & (ANGULAR_DRIVES | ANGULAR_MOTORS) != 0 || limit {
                let impulse = block.warm_impulse(3 + index, ANGULAR_DRIVES, ANGULAR_MOTORS, limit);
                block.angular[index].apply(&mut pair, impulse);
            }
        }
        if block.active & ACTIVE_CONE != 0 {
            block.limits[3].warm_start(&block.cone, &mut pair);
        }
        if layout & POSE_ANGULAR != 0 {
            block.angular_pose.warm_start_angular(&mut pair);
        }
        if layout & LINEAR_JACOBIANS != 0 {
            let linear = LinearRows::new(&geometry, frame);
            let limit = layout & LINEAR_ROWS != 0;
            for (index, jacobian) in linear.jacobians.iter().enumerate() {
                let impulse = block.warm_impulse(index, LINEAR_DRIVES, LINEAR_MOTORS, limit);
                jacobian.apply(&mut pair, impulse);
            }
        }
        if layout & POSE_LINEAR != 0 {
            let ((r_a, r_b), _) = pose_lever(&geometry, block.target_position);
            block.linear_pose.warm_start_linear(&mut pair, r_a, r_b);
        }
        if layout & ANGULAR_BLOCK != 0 {
            apply_rotation(&mut pair, block.block_impulse(3));
        }
        if layout & LINEAR_BLOCK != 0 {
            let ((r_a, r_b), _) = block.lock_lever(&geometry);
            apply_point(&mut pair, r_a, r_b, block.block_impulse(0));
        }
    }

    #[inline(always)]
    fn solve<W: Wide>(
        frame: &JointFrameW<W>,
        block: &mut AxesBlock<W>,
        a: &mut BodyW<W>,
        b: &mut BodyW<W>,
        ctx: &JointSolveContext,
    ) {
        let layout = block.layout;
        let zero = W::default();
        let geometry = JointGeometryW::new(frame, a, b);
        let relative = geometry.relative;
        let active = block.active;
        if active & ANGLE_LIMITS != 0 && block.far_from_limits(relative, (b.w - a.w).length(), ctx)
        {
            block.active &= !ANGLE_LIMITS;
        }
        let springs = layout & ANGULAR_SPRINGS != 0;
        let twist = if block.active & TWIST_LIMITS != 0 || springs {
            twist_angle(relative)
        } else {
            zero
        };
        let [swing_x, swing_y] = if block.active & SWING_LIMITS != 0 || springs {
            swing_angles(relative)
        } else {
            [zero; 2]
        };
        let angles = [swing_x, swing_y, twist];
        let fused = layout & POSES == 0;
        let mut pair = Pair { frame, a, b };
        let sides = if fused {
            [
                block.limit_sides(0),
                block.limit_sides(1),
                block.limit_sides(2),
            ]
        } else {
            [LimitSides::NONE; 3]
        };
        if layout & (ANGULAR_DRIVES | ANGULAR_MOTORS) != 0 || sides.iter().any(|side| side.any()) {
            block.solve_angular_rows(angles, sides, &mut pair, ctx);
        }
        if layout & POSE_ANGULAR != 0 {
            let error = geometry.rotation_error(block.target_rotation);
            let velocity = geometry.world_axis(block.angular_velocity);
            block
                .angular_pose
                .solve_angular(&mut pair, &block.rotation_mass, error, velocity);
        }
        let linear = LinearRows::when(layout & LINEAR_JACOBIANS != 0, &geometry, frame);
        if let Some(linear) = &linear {
            let limit = fused && layout & LINEAR_ROWS != 0;
            for index in 0..3 {
                block.solve_linear_axis(linear, index, limit, &mut pair, ctx);
            }
        }
        if layout & POSE_LINEAR != 0 {
            let (lever, error) = pose_lever(&geometry, block.target_position);
            let velocity = geometry.world_axis(block.linear_velocity);
            block
                .linear_pose
                .solve_linear(&mut pair, lever, error, velocity);
        }
        if !fused {
            for (index, angle) in angles.into_iter().enumerate() {
                if block.angular_limit(index) {
                    let ranged = index < 2 || layout & TWIST_RANGE != 0;
                    block.limits[3 + index].solve(
                        &block.angular[index],
                        angle,
                        ranged,
                        &mut pair,
                        ctx,
                    );
                }
            }
        }
        if layout & SWING != 0 && layout & SWING_RANGES == 0 {
            let errors = [
                swing_x - block.limits[3].lower,
                swing_y - block.limits[4].lower,
            ];
            let mut impulse = [block.limits[3].lower_impulse, block.limits[4].lower_impulse];
            block.swing_lock.solve(
                &mut pair,
                [&block.angular[0], &block.angular[1]],
                errors,
                &mut impulse,
                ctx.use_bias,
            );
            block.limits[3].lower_impulse = impulse[0];
            block.limits[4].lower_impulse = impulse[1];
        }
        if block.active & ACTIVE_CONE != 0 {
            let margin = block.cone_limit - cone_angle(relative);
            block.limits[3].solve_lower(&block.cone, margin, &mut pair, ctx);
        }
        if layout & ANGULAR_BLOCK != 0 {
            let error = geometry.rotation_error(block.locked_rotation);
            let mut impulse = block.block_impulse(3);
            solve_rotation_lock(
                &mut pair,
                &block.rotation_mass,
                error,
                &mut impulse,
                ctx.use_bias,
            );
            block.set_block_impulse(3, impulse);
        }
        if let Some(linear) = &linear
            && !fused
            && layout & LINEAR_ROWS != 0
        {
            for index in 0..3 {
                block.limits[index].solve(
                    &linear.jacobians[index],
                    linear.positions[index],
                    layout & LINEAR_RANGES != 0,
                    &mut pair,
                    ctx,
                );
            }
        }
        if layout & LINEAR_BLOCK != 0 {
            let (lever, error) = block.lock_lever(&geometry);
            let mut impulse = block.block_impulse(0);
            solve_point_lock(&mut pair, lever, error, &mut impulse, ctx.use_bias);
            block.set_block_impulse(0, impulse);
        }
        block.active = active;
    }

    #[inline(always)]
    fn impulses<W: Wide>(block: &AxesBlock<W>) -> [W; IMPULSE_SLOTS] {
        let layout = block.layout;
        let mut out = [W::default(); IMPULSE_SLOTS];
        for (index, limit) in block.limits.iter().enumerate() {
            out[LIMIT_LOWER + index] = limit.lower_impulse;
            out[LIMIT_UPPER + index] = limit.upper_impulse;
        }
        for (drives, motors, first) in [
            (LINEAR_DRIVES, LINEAR_MOTORS, 0),
            (ANGULAR_DRIVES, ANGULAR_MOTORS, 3),
        ] {
            for index in first..first + 3 {
                if layout & drives != 0 {
                    out[DRIVE + index] = block.drives[index].impulse;
                }
                if layout & motors != 0 {
                    out[MOTOR + index] = block.motors[index].impulse;
                }
            }
        }
        if layout & POSE_LINEAR != 0 {
            put_vec3(&mut out, POSE_LINEAR_IMPULSE, block.linear_pose.impulse);
        }
        if layout & POSE_ANGULAR != 0 {
            put_vec3(&mut out, POSE_ANGULAR_IMPULSE, block.angular_pose.impulse);
        }
        out
    }

    fn reaction(impulses: &JointImpulses, inv_h: f32) -> (f32, f32) {
        let rows = |first: usize| {
            impulses.vec3(LIMIT_LOWER + first) - impulses.vec3(LIMIT_UPPER + first)
                + impulses.vec3(DRIVE + first)
                + impulses.vec3(MOTOR + first)
        };
        let force = rows(0).length() + impulses.vec3(POSE_LINEAR_IMPULSE).length();
        let torque = rows(3).length() + impulses.vec3(POSE_ANGULAR_IMPULSE).length();
        (force * inv_h, torque * inv_h)
    }
}
