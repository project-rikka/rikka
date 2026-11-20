use rikka_physics_collision::BodyFrame;
use rikka_physics_core::Shared;
use rikka_physics_math::{Quat, QuatW, Transform, Vec3W, Wide};

use super::columns::store_rows;
use super::impulses::JointImpulses;
use super::model::{JointBlock, JointFrameW, JointModel, JointPrepareContext, JointSolveContext};
use super::record::JointRecord;
use crate::set::BodyMotion;
use crate::solver::contact::ContactPass;
use crate::solver::lanes::{
    BodyW, LaneIndices, LaneKinds, MAX_LANES, States, load_masses, transpose,
};
use crate::solver::softness::Softness;
use crate::solver::wide_buffer::Plain;

pub(crate) const NO_JOINT: u32 = u32::MAX;

#[inline(always)]
fn quats<W: Wide>(quats: [&Quat; MAX_LANES]) -> QuatW<W> {
    let [x, y, z, s] =
        unsafe { transpose::<W>(&quats.map(|quat| std::ptr::from_ref(quat).cast()), 0) };
    QuatW::new(Vec3W::new(x, y, z), s)
}

#[inline(always)]
fn centers<W: Wide>(bodies: &[&BodyFrame; MAX_LANES]) -> Vec3W<W> {
    const {
        assert!(
            std::mem::offset_of!(BodyFrame, center)
                == std::mem::offset_of!(BodyFrame, transform)
                    + std::mem::offset_of!(Transform, q)
                    + size_of::<Quat>()
        )
    };
    let [_, x, y, z] = unsafe {
        transpose::<W>(
            &bodies.map(|body| std::ptr::from_ref(&body.transform.q).cast::<f32>()),
            3,
        )
    };
    Vec3W::new(x, y, z)
}

struct FrameSide<W: Wide> {
    anchor: Vec3W<W>,
    rotation: QuatW<W>,
    center: Vec3W<W>,
}

impl<W: Wide> FrameSide<W> {
    #[inline(always)]
    fn new(
        frames: &[BodyFrame],
        records: &[&JointRecord; MAX_LANES],
        side: fn(&JointRecord) -> (u32, &Transform),
    ) -> Self {
        let mut bodies = [&frames[side(records[0]).0 as usize]; MAX_LANES];
        let mut locals = [&Transform::IDENTITY; MAX_LANES];
        for ((body, local), record) in bodies.iter_mut().zip(&mut locals).zip(records) {
            let (index, frame) = side(record);
            *body = &frames[index as usize];
            *local = frame;
        }
        let turn = quats::<W>(bodies.map(|body| &body.transform.q));
        let position = Vec3W::from_fn(|lane| bodies[lane].transform.p);
        let center = centers::<W>(&bodies);
        let anchor = Vec3W::from_fn(|lane| locals[lane].p);
        Self {
            anchor: turn.rotate(anchor) + position - center,
            rotation: turn * quats::<W>(locals.map(|local| &local.q)),
            center,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct JointLanes {
    pub index_a: LaneIndices,
    pub index_b: LaneIndices,
    pub joints: [u32; MAX_LANES],
    pub kinds_a: LaneKinds,
    pub kinds_b: LaneKinds,
}

unsafe impl Plain for JointLanes {}

#[derive(Clone, Copy, Debug)]
pub(crate) struct JointContext<'a> {
    pub joints: Shared<'a, JointRecord>,
    pub frames: &'a [BodyFrame],
    pub motion: Shared<'a, BodyMotion>,
    pub split: &'a [f32],
    pub soft: Softness,
    pub prepare: JointPrepareContext,
    pub states: &'a States<'a>,
}

#[inline(always)]
pub(crate) fn prepare_batch<W: Wide, M: JointModel>(
    block: &mut JointBlock<W, M>,
    lanes: &mut JointLanes,
    ids: &[u32],
    ctx: &JointContext<'_>,
) {
    let first = unsafe { &*ctx.joints.as_ptr().add(ids[0] as usize) };
    let mut params = [M::params(&first.params); MAX_LANES];
    let mut impulses = [&JointImpulses::ZERO; MAX_LANES];
    let mut records = [first; MAX_LANES];
    *lanes = JointLanes {
        index_a: [0; MAX_LANES],
        index_b: [0; MAX_LANES],
        joints: [NO_JOINT; MAX_LANES],
        kinds_a: LaneKinds::default(),
        kinds_b: LaneKinds::default(),
    };
    let count = ids.len().min(W::LANES);
    for (lane, &id) in ids.iter().enumerate().take(count) {
        let record = unsafe { &*ctx.joints.as_ptr().add(id as usize) };
        params[lane] = M::params(&record.params);
        impulses[lane] = &record.impulses;
        lanes.index_a[lane] = record.solver_a;
        lanes.index_b[lane] = record.solver_b;
        lanes.joints[lane] = id;
        records[lane] = record;
    }
    lanes.kinds_a = ctx.states.kinds::<W>(&lanes.index_a);
    lanes.kinds_b = ctx.states.kinds::<W>(&lanes.index_b);
    let a = FrameSide::<W>::new(ctx.frames, &records, |record| {
        (record.body_a, &record.frame_a)
    });
    let b = FrameSide::<W>::new(ctx.frames, &records, |record| {
        (record.body_b, &record.frame_b)
    });
    let (inv_mass_a, inv_i_a) = load_masses::<W>(ctx.motion, ctx.split, &lanes.index_a);
    let (inv_mass_b, inv_i_b) = load_masses::<W>(ctx.motion, ctx.split, &lanes.index_b);
    let (bias_rate, mass_scale, impulse_scale) = if records
        .iter()
        .take(count)
        .all(|record| record.softness.is_none())
    {
        (
            W::splat(ctx.soft.bias_rate),
            W::splat(ctx.soft.mass_scale),
            W::splat(ctx.soft.impulse_scale),
        )
    } else {
        let mut soft = [ctx.soft; MAX_LANES];
        for (lane, record) in records.iter().enumerate().take(count) {
            if let Some(spring) = record.softness {
                let m = inv_mass_a.lane(lane) + inv_mass_b.lane(lane);
                let effective = if m > 0.0 { 1.0 / m } else { 0.0 };
                soft[lane] =
                    spring.softness(effective, ctx.prepare.h, ctx.prepare.max_spring_hertz);
            }
        }
        (
            W::from_fn(|lane| soft[lane].bias_rate),
            W::from_fn(|lane| soft[lane].mass_scale),
            W::from_fn(|lane| soft[lane].impulse_scale),
        )
    };
    block.frame = JointFrameW {
        anchor_a: a.anchor,
        anchor_b: b.anchor,
        rotation_a: a.rotation,
        rotation_b: b.rotation,
        delta_center: b.center - a.center,
        inv_mass_a,
        inv_mass_b,
        inv_i_a,
        inv_i_b,
        bias_rate,
        mass_scale,
        impulse_scale,
    };
    M::prepare(
        &mut block.model,
        &block.frame,
        &params,
        &impulses,
        &ctx.prepare,
    );
}

#[inline(always)]
pub(crate) fn solve_pass<W: Wide, M: JointModel>(
    block: &mut JointBlock<W, M>,
    a: &mut BodyW<W>,
    b: &mut BodyW<W>,
    pass: ContactPass,
    ctx: &JointSolveContext,
) {
    let solve = JointSolveContext {
        use_bias: pass == ContactPass::Solve,
        ..*ctx
    };
    if pass == ContactPass::Warm {
        M::warm_start(&block.frame, &block.model, a, b);
    } else {
        M::solve(&block.frame, &mut block.model, a, b, &solve);
    }
}

#[inline(always)]
pub(crate) fn solve_batch<W: Wide, M: JointModel>(
    block: &mut JointBlock<W, M>,
    lanes: &JointLanes,
    states: &States<'_>,
    pass: ContactPass,
    ctx: &JointSolveContext,
) {
    let mut a = states.gather_kinds::<W>(&lanes.index_a, lanes.kinds_a);
    let mut b = states.gather_kinds::<W>(&lanes.index_b, lanes.kinds_b);
    solve_pass::<W, M>(block, &mut a, &mut b, pass, ctx);
    unsafe {
        states.scatter_kinds(&lanes.index_a, lanes.kinds_a, &a.v, &a.w);
        states.scatter_kinds(&lanes.index_b, lanes.kinds_b, &b.v, &b.w);
    }
}

#[inline(always)]
pub(crate) fn store_batch<W: Wide, M: JointModel>(
    block: &JointBlock<W, M>,
    lanes: &JointLanes,
    joints: Shared<'_, JointRecord>,
    inv_h: f32,
) {
    let mut sinks = [JointImpulses::ZERO; MAX_LANES];
    let mut rows = sinks.each_mut().map(|sink| sink.0.as_mut_ptr());
    for (row, &id) in rows.iter_mut().zip(&lanes.joints).take(W::LANES) {
        if id != NO_JOINT {
            let record = unsafe { joints.get(id as usize) };
            record.reaction_scale = inv_h;
            *row = record.impulses.0.as_mut_ptr();
        }
    }
    unsafe { store_rows(&M::impulses(&block.model), &rows) };
}
