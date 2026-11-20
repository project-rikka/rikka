use rikka_physics_core::Block;
use rikka_physics_math::{F32x1, Kernel, Wide};

use super::contact::{
    ContactBlock, ContactContext, ContactLanes, ContactPass, prepare, solve_block, solve_pass,
    store,
};
use super::context::StepContext;
use super::finalize::finalize_bodies;
use super::integrate::{integrate_positions, integrate_velocities};
use super::joint_kernels::{JointStage, run_joints, write_delta};
use super::lanes::States;
use super::plan::{ColorPlan, OVERFLOW_BLOCK, StageOp, TaskStep};

#[inline(always)]
fn contact_batch<'a, W: Wide, V: Wide>(
    ctx: &StepContext<'a>,
    batch: u32,
) -> (&'a mut ContactBlock<V>, &'a mut ContactLanes) {
    let index = batch as usize;
    unsafe {
        (
            &mut *ctx
                .contact_blocks
                .at::<ContactBlock<V>>(index, size_of::<ContactBlock<W>>()),
            &mut *ctx
                .contact_lanes
                .at::<ContactLanes>(index, size_of::<ContactLanes>()),
        )
    }
}

#[inline(always)]
fn prepare_contacts<W: Wide>(ctx: &StepContext<'_>, batch: u32) {
    let ids = ctx.plan.contact_batch_ids(batch);
    if ctx.plan.overflow.contacts.contains(&batch) {
        let split = ContactContext {
            split: &ctx.plan.overflow_split,
            ..ctx.contact
        };
        let (block, lanes) = contact_batch::<W, F32x1>(ctx, batch);
        prepare(block, lanes, ids, &split);
    } else if ids.len() == 1 {
        let (block, lanes) = contact_batch::<W, F32x1>(ctx, batch);
        prepare(block, lanes, ids, &ctx.contact);
    } else {
        let (block, lanes) = contact_batch::<W, W>(ctx, batch);
        prepare(block, lanes, ids, &ctx.contact);
    }
}
#[inline(always)]
fn solve_contacts_as<W: Wide, V: Wide>(
    ctx: &StepContext<'_>,
    states: &States<'_>,
    batch: u32,
    pass: ContactPass,
) {
    let (block, lanes) = contact_batch::<W, V>(ctx, batch);
    solve_block(block, lanes, states, &ctx.iteration, pass);
    if pass == ContactPass::RelaxRestitution {
        store(block, lanes, &ctx.contact);
    }
}

#[inline(always)]
fn solve_contacts<W: Wide>(
    ctx: &StepContext<'_>,
    states: &States<'_>,
    batch: u32,
    pass: ContactPass,
) {
    if ctx.plan.contact_counts[batch as usize] == 1 {
        solve_contacts_as::<W, F32x1>(ctx, states, batch, pass);
    } else {
        solve_contacts_as::<W, W>(ctx, states, batch, pass);
    }
}
#[inline(always)]
fn solve_color<W: Wide>(
    ctx: &StepContext<'_>,
    color: &ColorPlan,
    range: std::ops::Range<u32>,
    pass: ContactPass,
) {
    let states = &ctx.integrate.states;
    let contacts = color.contacts.len() as u32;
    for index in range {
        if index < contacts {
            solve_contacts::<W>(ctx, states, color.contacts.start + index, pass);
        } else {
            run_joints(
                ctx,
                states,
                color.joints.start + index - contacts,
                JointStage::Solve(pass),
            );
        }
    }
}

#[inline(always)]
fn run_sequence<W: Wide>(
    ctx: &StepContext<'_>,
    sequence: &[TaskStep],
    states: &States<'_>,
    pass: ContactPass,
) {
    for step in sequence {
        match *step {
            TaskStep::Contacts(batch) => solve_contacts::<W>(ctx, states, batch, pass),
            TaskStep::Joints(batch) => run_joints(ctx, states, batch, JointStage::Solve(pass)),
        }
    }
}

#[inline(always)]
fn prepare_task<W: Wide>(ctx: &StepContext<'_>, sequence: &[TaskStep]) {
    for step in sequence {
        match *step {
            TaskStep::Contacts(batch) => prepare_contacts::<W>(ctx, batch),
            TaskStep::Joints(batch) => {
                run_joints(ctx, &ctx.integrate.states, batch, JointStage::Prepare)
            }
        }
    }
}

#[inline(always)]
fn solve_task<W: Wide>(ctx: &StepContext<'_>, bodies: &[u32], sequence: &[TaskStep], substep: u32) {
    integrate_velocities::<W>(bodies, &ctx.integrate);
    let states = ctx
        .integrate
        .states
        .with_kinematic_steps(substep, ctx.integrate.h);
    run_sequence::<W>(ctx, sequence, &states, ContactPass::Warm);
    run_sequence::<W>(ctx, sequence, &states, ContactPass::Solve);
    integrate_positions::<W>(bodies, &ctx.integrate);
}

#[inline(always)]
fn relax_task<W: Wide>(ctx: &StepContext<'_>, sequence: &[TaskStep], substep: u32) {
    let states = ctx
        .integrate
        .states
        .with_kinematic_steps(substep + 1, ctx.integrate.h);
    let pass = if substep + 1 == ctx.substeps {
        ContactPass::RelaxRestitution
    } else {
        ContactPass::Relax
    };
    run_sequence::<W>(ctx, sequence, &states, pass);
}

#[inline(always)]
fn run_task<W: Wide>(ctx: &StepContext<'_>, task: usize) {
    let plan = ctx.plan;
    let task = &plan.tasks[task];
    let bodies = &plan.task_bodies[task.bodies.start as usize..task.bodies.end as usize];
    let sequence = &plan.task_sequence[task.sequence.start as usize..task.sequence.end as usize];
    prepare_task::<W>(ctx, sequence);
    for substep in 0..ctx.substeps {
        solve_task::<W>(ctx, bodies, sequence, substep);
        relax_task::<W>(ctx, sequence, substep);
    }
}

pub(crate) struct StageKernel<'a, 'b> {
    pub ctx: &'a StepContext<'b>,
    pub op: StageOp,
    pub block: Block,
}

impl Kernel for StageKernel<'_, '_> {
    type Output = ();

    #[inline(always)]
    fn run<W: Wide>(self) {
        let ctx = self.ctx;
        let plan = ctx.plan;
        let index = self.block.index as usize;
        match self.op {
            StageOp::Prepare => {
                let total = plan.colored_contacts as usize + plan.colored_joints as usize;
                for batch in self.block.range(plan.prepare_width, total) {
                    if batch < plan.colored_contacts as usize {
                        prepare_contacts::<W>(ctx, batch as u32);
                    } else {
                        run_joints(
                            ctx,
                            &ctx.integrate.states,
                            (batch - plan.colored_contacts as usize) as u32,
                            JointStage::Prepare,
                        );
                    }
                }
            }
            StageOp::IslandTasks => run_task::<W>(ctx, index),
            StageOp::IntegrateVelocities => {
                let bodies = &plan.colored_bodies
                    [self.block.range(plan.body_width, plan.colored_bodies.len())];
                integrate_velocities::<W>(bodies, &ctx.integrate);
            }
            StageOp::IntegratePositions => {
                let bodies = &plan.colored_bodies
                    [self.block.range(plan.body_width, plan.colored_bodies.len())];
                integrate_positions::<W>(bodies, &ctx.integrate);
            }
            StageOp::Colors {
                first,
                count,
                pass,
                width,
            } => {
                if count == 1 {
                    let color = &plan.colors[first as usize];
                    let range = self.block.range(width as usize, color.batches() as usize);
                    solve_color::<W>(ctx, color, range.start as u32..range.end as u32, pass);
                } else {
                    for color in &plan.colors[first as usize..(first + count) as usize] {
                        solve_color::<W>(ctx, color, 0..color.batches(), pass);
                    }
                }
            }
            StageOp::OverflowSolve { pass } => {
                let contacts = plan.overflow.contacts.len();
                for local in self
                    .block
                    .range(OVERFLOW_BLOCK, plan.overflow.batches() as usize)
                {
                    if local < contacts {
                        jacobi_contacts::<W>(
                            ctx,
                            local,
                            plan.overflow.contacts.start + local as u32,
                            pass,
                        );
                    } else {
                        run_joints(
                            ctx,
                            &ctx.integrate.states,
                            plan.overflow.joints.start + (local - contacts) as u32,
                            JointStage::Jacobi { local, pass },
                        );
                    }
                }
            }
            StageOp::OverflowApply => {
                for index in self.block.range(OVERFLOW_BLOCK, plan.overflow_bodies.len()) {
                    apply_overflow(ctx, index);
                }
            }
            StageOp::Finalize => {
                let range = self
                    .block
                    .range(plan.body_width, plan.awake_bodies as usize);
                finalize_bodies(ctx, range, self.block.worker);
            }
        }
    }
}

#[inline(always)]
fn jacobi_contacts<W: Wide>(ctx: &StepContext<'_>, local: usize, batch: u32, pass: ContactPass) {
    let (block, lanes) = contact_batch::<W, F32x1>(ctx, batch);
    let states = &ctx.integrate.states;
    let mut a = states.gather_kinds::<F32x1>(&lanes.index_a, lanes.kinds_a);
    let mut b = states.gather_kinds::<F32x1>(&lanes.index_b, lanes.kinds_b);
    let before = [a.v.lane(0), a.w.lane(0), b.v.lane(0), b.w.lane(0)];
    solve_pass(block, lanes, &mut a, &mut b, &ctx.iteration, pass);
    if pass == ContactPass::RelaxRestitution {
        store(block, lanes, &ctx.contact);
    }
    write_delta(
        ctx,
        local,
        before,
        [a.v.lane(0), a.w.lane(0), b.v.lane(0), b.w.lane(0)],
    );
}
fn apply_overflow(ctx: &StepContext<'_>, index: usize) {
    let plan = ctx.plan;
    let body = plan.overflow_bodies[index];
    let entries = &plan.overflow_entries
        [plan.overflow_offsets[index] as usize..plan.overflow_offsets[index + 1] as usize];
    let mut linear = rikka_physics_math::Vec3::ZERO;
    let mut angular = rikka_physics_math::Vec3::ZERO;
    for &entry in entries {
        let delta = unsafe { ctx.overflow_deltas.at((entry >> 1) as usize) };
        let side = (entry & 1) as usize * 2;
        linear += delta[side];
        angular += delta[side + 1];
    }
    let scale = 1.0 / entries.len() as f32;
    let state = unsafe { ctx.integrate.states.state(body) };
    state.v += linear * scale;
    state.w += angular * scale;
}
