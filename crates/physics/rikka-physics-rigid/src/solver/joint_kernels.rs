use std::marker::PhantomData;

use rikka_physics_math::{F32x1, Kernel, Vec3, Wide};

use super::contact::ContactPass;
use super::context::StepContext;
use super::lanes::States;
use crate::joint::{
    JointBlock, JointContext, JointLanes, JointModel, JointRecord, JointVisitor, prepare_batch,
    solve_batch, solve_pass, store_batch,
};

#[derive(Clone, Copy, Debug)]
pub(super) enum JointStage {
    Prepare,
    Solve(ContactPass),
    Jacobi { local: usize, pass: ContactPass },
}

#[inline(always)]
fn joint_block<'a, W: Wide, V: Wide, M: JointModel>(
    ctx: &StepContext<'a>,
    batch: u32,
) -> (&'a mut JointBlock<V, M>, &'a mut JointLanes) {
    let entry = ctx.plan.joint_batches[batch as usize];
    unsafe {
        (
            &mut *ctx.joint_blocks[entry.kind.index()]
                .at::<JointBlock<V, M>>(entry.slot as usize, size_of::<JointBlock<W, M>>()),
            &mut *ctx
                .joint_lanes
                .at::<JointLanes>(batch as usize, size_of::<JointLanes>()),
        )
    }
}

#[inline(always)]
pub(super) fn write_delta(
    ctx: &StepContext<'_>,
    local: usize,
    before: [Vec3; 4],
    after: [Vec3; 4],
) {
    let slot = unsafe { ctx.overflow_deltas.get(local) };
    for side in 0..4 {
        slot[side] = after[side] - before[side];
    }
}

#[inline(always)]
fn prepare<'b, W: Wide, V: Wide, M: JointModel>(
    ctx: &StepContext<'b>,
    states: &States<'b>,
    batch: u32,
) {
    let (block, lanes) = joint_block::<W, V, M>(ctx, batch);
    let joint_ctx = JointContext {
        joints: ctx.joints,
        frames: unsafe { std::slice::from_raw_parts(ctx.frames.as_ptr(), ctx.frames.len()) },
        motion: ctx.integrate.motion,
        split: if ctx.plan.overflow.joints.contains(&batch) {
            &ctx.plan.overflow_split
        } else {
            &[]
        },
        soft: ctx.joint_soft,
        prepare: ctx.joint_prepare,
        states,
    };
    if batch as usize + 1 < ctx.plan.joint_counts.len() {
        for &id in ctx.plan.joint_batch_ids(batch + 1) {
            ctx.joints.prefetch(id as usize, size_of::<JointRecord>());
        }
    }
    prepare_batch::<V, M>(block, lanes, ctx.plan.joint_batch_ids(batch), &joint_ctx);
}

#[inline(always)]
fn solve<W: Wide, V: Wide, M: JointModel>(
    ctx: &StepContext<'_>,
    states: &States<'_>,
    batch: u32,
    pass: ContactPass,
) {
    let (block, lanes) = joint_block::<W, V, M>(ctx, batch);
    solve_batch::<V, M>(block, lanes, states, pass, &ctx.joint_solve);
    if pass == ContactPass::RelaxRestitution {
        store_batch::<V, M>(block, lanes, ctx.joints, ctx.joint_prepare.inv_h);
    }
}

#[inline(always)]
fn jacobi<W: Wide, M: JointModel>(
    ctx: &StepContext<'_>,
    local: usize,
    batch: u32,
    pass: ContactPass,
) {
    let (block, lanes) = joint_block::<W, F32x1, M>(ctx, batch);
    let states = &ctx.integrate.states;
    let mut a = states.gather::<F32x1>(&lanes.index_a);
    let mut b = states.gather::<F32x1>(&lanes.index_b);
    let before = [a.v.lane(0), a.w.lane(0), b.v.lane(0), b.w.lane(0)];
    solve_pass::<F32x1, M>(block, &mut a, &mut b, pass, &ctx.joint_solve);
    if pass == ContactPass::RelaxRestitution {
        store_batch::<F32x1, M>(block, lanes, ctx.joints, ctx.joint_prepare.inv_h);
    }
    write_delta(
        ctx,
        local,
        before,
        [a.v.lane(0), a.w.lane(0), b.v.lane(0), b.w.lane(0)],
    );
}

struct JointOp<'a, 'b, M> {
    ctx: &'a StepContext<'b>,
    states: &'a States<'b>,
    batch: u32,
    stage: JointStage,
    model: PhantomData<M>,
}

impl<M: JointModel> Kernel for JointOp<'_, '_, M> {
    type Output = ();

    #[inline(always)]
    fn run<W: Wide>(self) {
        let (ctx, batch) = (self.ctx, self.batch);
        let single = ctx.plan.joint_counts[batch as usize] == 1;
        match self.stage {
            JointStage::Prepare if single => prepare::<W, F32x1, M>(ctx, self.states, batch),
            JointStage::Prepare => prepare::<W, W, M>(ctx, self.states, batch),
            JointStage::Solve(pass) if single => {
                solve::<W, F32x1, M>(ctx, self.states, batch, pass)
            }
            JointStage::Solve(pass) => solve::<W, W, M>(ctx, self.states, batch, pass),
            JointStage::Jacobi { local, pass } => jacobi::<W, M>(ctx, local, batch, pass),
        }
    }
}

struct JointDispatch<'a, 'b> {
    ctx: &'a StepContext<'b>,
    states: &'a States<'b>,
    batch: u32,
    stage: JointStage,
}

impl JointVisitor for JointDispatch<'_, '_> {
    type Output = ();

    #[inline(always)]
    fn visit<M: JointModel>(self) {
        self.ctx.simd.run(JointOp::<M> {
            ctx: self.ctx,
            states: self.states,
            batch: self.batch,
            stage: self.stage,
            model: PhantomData,
        });
    }
}

#[inline(always)]
pub(super) fn run_joints<'b>(
    ctx: &StepContext<'b>,
    states: &States<'b>,
    batch: u32,
    stage: JointStage,
) {
    ctx.plan.joint_batches[batch as usize]
        .kind
        .dispatch(JointDispatch {
            ctx,
            states,
            batch,
            stage,
        });
}
