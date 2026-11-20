use std::marker::PhantomData;
use std::ops::Range;

use rikka_physics_core::{Block, PerWorker, Shared};
use rikka_physics_math::{Kernel, Wide};

use super::contact::{ContactBlock, ContactContext, ContactLanes, IterationContext};
use super::context::{FinalizeContext, FinalizeOutput, RawBuffer, StepContext};
use super::integrate::IntegrateContext;
use super::kernels::StageKernel;
use super::lanes::States;
use super::softness::Softness;
use crate::config::Config;
use crate::joint::{
    JointBlock, JointKind, JointLanes, JointModel, JointPrepareContext, JointSolveContext,
    JointVisitor,
};
use crate::set::AWAKE_SET;
use crate::world::RigidWorld;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct StepParams {
    pub dt: f32,
    pub inv_dt: f32,
    pub h: f32,
    pub inv_h: f32,
    pub contact: Softness,
    pub static_contact: Softness,
    pub joint: Softness,
    pub max_spring_hertz: f32,
}

impl StepParams {
    pub(crate) fn new(config: &Config, dt: f32) -> Self {
        let substeps = config.substeps as f32;
        let h = dt / substeps;
        let inv_h = if h > 0.0 { 1.0 / h } else { 0.0 };
        let (contact_hertz, contact_ratio) = config.contact_softness.frequency(1.0);
        let contact_hertz = contact_hertz.min(0.125 * inv_h);
        let (joint_hertz, joint_ratio) = config.joint_softness.frequency(1.0);
        Self {
            dt,
            inv_dt: if dt > 0.0 { 1.0 / dt } else { 0.0 },
            h,
            inv_h,
            contact: Softness::new(contact_hertz, contact_ratio, h),
            static_contact: Softness::new(2.0 * contact_hertz, 0.5 * contact_ratio, h),
            joint: Softness::new(joint_hertz.min(0.25 * inv_h), joint_ratio, h),
            max_spring_hertz: 0.25 * inv_h,
        }
    }
}

struct BlockSizes;

impl Kernel for BlockSizes {
    type Output = (usize, [usize; JointKind::COUNT]);

    fn run<W: Wide>(self) -> Self::Output {
        let joints = std::array::from_fn(|kind| {
            JointKind::ALL[kind].dispatch(JointBlockSize::<W>(PhantomData))
        });
        (size_of::<ContactBlock<W>>(), joints)
    }
}

struct JointBlockSize<W>(PhantomData<W>);

impl<W: Wide> JointVisitor for JointBlockSize<W> {
    type Output = usize;

    fn visit<M: JointModel>(self) -> usize {
        size_of::<JointBlock<W, M>>()
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RawBuffers {
    pub contact_blocks: RawBuffer,
    pub contact_lanes: RawBuffer,
    pub joint_blocks: [RawBuffer; JointKind::COUNT],
    pub joint_lanes: RawBuffer,
}

impl RigidWorld {
    pub(crate) fn reserve_buffers(&mut self) -> RawBuffers {
        let (contact_size, joint_sizes) = self.simd.run(BlockSizes);
        let solver = &mut self.solver;
        let plan = &solver.plan;
        let batches = plan.contact_batches as usize;
        let contact_blocks = solver.contact_blocks.reserve(batches * contact_size);
        let contact_lanes = solver
            .contact_lanes
            .reserve(batches * size_of::<ContactLanes>());
        let joint_blocks = std::array::from_fn(|kind| {
            solver.joint_blocks[kind].reserve(plan.joint_slots[kind] as usize * joint_sizes[kind])
        });
        let joint_lanes = solver
            .joint_lanes
            .reserve(plan.joint_batches.len() * size_of::<JointLanes>());
        let workers = self.executor.workers();
        if solver.outputs.len() != workers {
            solver.outputs = PerWorker::new(workers, |_| FinalizeOutput::default());
        }
        let islands = self.islands.records.len();
        for output in solver.outputs.iter_mut() {
            output.enlarged.clear();
            output.fast.clear();
            output.bullets.clear();
            output.moving_kinematic.clear();
            output.awake_islands.clear_all();
            output.awake_islands.grow(islands);
        }
        RawBuffers {
            contact_blocks,
            contact_lanes,
            joint_blocks,
            joint_lanes,
        }
    }

    pub(crate) fn run_stages(&mut self, range: Range<usize>, buffers: &RawBuffers) {
        if range.is_empty() {
            return;
        }
        let params = self.solver.params;
        let config = self.config;
        let executor = self.executor.clone();
        let simd = self.simd;
        let awake = &mut self.sets.sets[AWAKE_SET as usize];
        let body_ids = &awake.bodies;
        let states = States::new(Shared::new(&mut awake.states));
        let motion = Shared::new(&mut awake.motion);
        let poses = Shared::new(&mut awake.poses);
        let geometry = Shared::new(&mut self.geometry);
        let mut extras = std::mem::take(&mut self.solver.extras);
        self.narrowphase.pools_mut().views(&mut extras);
        let joints = Shared::new(&mut self.joints.records);
        let frames = Shared::new(&mut self.frames);
        if self.events.body_moves.len() < body_ids.len() {
            self.events.body_moves.resize(
                body_ids.len(),
                crate::BodyMoveEvent {
                    body: crate::BodyId {
                        index: 0,
                        generation: 0,
                    },
                    transform: rikka_physics_math::Transform::IDENTITY,
                    user_data: 0,
                    fell_asleep: false,
                },
            );
        }
        let overflow = self.solver.plan.overflow.batches() as usize;
        if self.solver.overflow_deltas.len() < overflow {
            self.solver
                .overflow_deltas
                .resize(overflow, [rikka_physics_math::Vec3::ZERO; 4]);
        }
        let overflow_deltas = Shared::new(&mut self.solver.overflow_deltas);
        let (body_records, generations) = self.bodies.split_mut();
        let plan = &self.solver.plan;
        let ctx = StepContext {
            plan,
            simd,
            integrate: IntegrateContext {
                states,
                motion,
                poses,
                gravity: config.gravity,
                h: params.h,
                max_linear_speed: config.max_linear_speed * config.tolerance.length_scale,
                max_angular_speed: rikka_physics_geometry::Tolerance::MAX_ROTATION * params.inv_h,
            },
            poses,
            frames,
            contact: ContactContext {
                geometry,
                headers: &self.headers,
                extras: &extras,
                contacts: &self.contacts.records,
                motion,
                states,
                soft: params.contact,
                static_soft: params.static_contact,
                warm_start: if config.warm_starting { 1.0 } else { 0.0 },
                speculative: config.tolerance.speculative_distance(),
                split: &[],
            },
            iteration: IterationContext {
                inv_h: params.inv_h,
                contact_speed: config.contact_push_speed * config.tolerance.length_scale,
                restitution_threshold: config.restitution_threshold * config.tolerance.length_scale,
            },
            joints,
            joint_prepare: JointPrepareContext {
                h: params.h,
                inv_h: params.inv_h,
                max_spring_hertz: params.max_spring_hertz,
                warm_start: if config.warm_starting { 1.0 } else { 0.0 },
            },
            joint_solve: JointSolveContext {
                inv_h: params.inv_h,
                use_bias: true,
            },
            joint_soft: params.joint,
            contact_blocks: buffers.contact_blocks,
            contact_lanes: buffers.contact_lanes,
            joint_blocks: buffers.joint_blocks,
            joint_lanes: buffers.joint_lanes,
            overflow_deltas,
            substeps: config.substeps,
            finalize: FinalizeContext {
                bodies: Shared::new(body_records),
                body_ids,
                shapes: &self.shapes.records,
                colliders: &self.colliders,
                fat: Shared::new(self.broadphase.fat_bounds_mut()),
                moves: Shared::new(&mut self.events.body_moves),
                generations,
                output: &self.solver.outputs,
                dt: params.dt,
                inv_dt: params.inv_dt,
                sleeping: config.sleeping,
                time_to_sleep: config.time_to_sleep,
                slop: config.linear_slop(),
                speculative_cap: config.speculative_cap(),
                tolerance: config.tolerance,
            },
        };
        let ops = &plan.ops[range.clone()];
        let task = |block: Block| {
            simd.run(StageKernel {
                ctx: &ctx,
                op: ops[block.stage as usize],
                block,
            });
        };
        executor.run(&plan.stages[range], &task);
        self.solver.extras = extras;
    }

    pub(crate) fn collect_finalize(&mut self) {
        let solver = &mut self.solver;
        solver.enlarged.clear();
        solver.fast_regular.clear();
        solver.fast_bullets.clear();
        solver.moving_kinematic.clear();
        solver.awake_islands.clear_all();
        solver.awake_islands.grow(self.islands.records.len());
        for output in solver.outputs.iter_mut() {
            solver.enlarged.append(&mut output.enlarged);
            solver.fast_regular.append(&mut output.fast);
            solver.fast_bullets.append(&mut output.bullets);
            solver.moving_kinematic.append(&mut output.moving_kinematic);
            solver.awake_islands.union_with(&output.awake_islands);
        }
        for output in solver.outputs.iter_mut() {
            output.enlarged.reserve(solver.enlarged.len());
            output.fast.reserve(solver.fast_regular.len());
            output.bullets.reserve(solver.fast_bullets.len());
            output
                .moving_kinematic
                .reserve(solver.moving_kinematic.len());
        }
        solver.moving_kinematic.sort_unstable();
    }
}
