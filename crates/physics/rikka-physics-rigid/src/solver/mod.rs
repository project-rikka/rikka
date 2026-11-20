mod build;
pub(crate) mod contact;
mod context;
mod continuous;
pub(crate) mod execute;
mod finalize;
mod gather;
mod integrate;
mod joint_kernels;
mod kernels;
pub(crate) mod lanes;
mod plan;
mod schedule;
pub(crate) mod softness;
pub(crate) mod state;
pub(crate) mod sym;
pub(crate) mod wide_buffer;

use rikka_physics_core::{BitSet, PerWorker};

pub(crate) use execute::StepParams;

use crate::joint::JointKind;
use build::BuildScratch;
use context::FinalizeOutput;
use plan::Plan;
use schedule::Scheduler;
use wide_buffer::WideBuffer;

#[derive(Debug)]
pub(crate) struct Solver {
    pub plan: Plan,
    pub build: BuildScratch,
    pub scheduler: Scheduler,
    pub params: StepParams,
    contact_blocks: WideBuffer,
    contact_lanes: WideBuffer,
    joint_blocks: [WideBuffer; JointKind::COUNT],
    joint_lanes: WideBuffer,
    outputs: PerWorker<FinalizeOutput>,
    pub enlarged: Vec<u32>,
    pub impact_enlarged: Vec<u32>,
    pub awake_islands: BitSet,
    pub moving_kinematic: Vec<u32>,
    pub moving_before: Vec<u32>,
    pub moving_bits: BitSet,
    pub targets: Vec<u32>,
    pub split_bodies: Vec<u32>,
    pub split_contacts: Vec<u32>,
    pub split_joints: Vec<u32>,
    pub split_stack: Vec<u32>,
    pub new_pairs: Vec<u64>,
    pub new_contacts: Vec<u32>,
    pub changes: Vec<u32>,
    pub skipped: Vec<u32>,
    pub extras: Vec<rikka_physics_collision::PoolView>,
    pub overflow_deltas: Vec<[rikka_physics_math::Vec3; 4]>,
    pub fast_regular: Vec<u32>,
    pub fast_bullets: Vec<u32>,
    pub continuous_enlarged: PerWorker<Vec<u32>>,
    pub pre_solve_skipped: PerWorker<Vec<u32>>,
}

impl Default for Solver {
    fn default() -> Self {
        Self {
            plan: Plan::default(),
            build: BuildScratch::default(),
            scheduler: Scheduler::default(),
            params: StepParams::default(),
            contact_blocks: WideBuffer::default(),
            contact_lanes: WideBuffer::default(),
            joint_blocks: Default::default(),
            joint_lanes: WideBuffer::default(),
            outputs: PerWorker::new(1, |_| FinalizeOutput::default()),
            enlarged: Vec::new(),
            impact_enlarged: Vec::new(),
            awake_islands: BitSet::default(),
            moving_kinematic: Vec::new(),
            moving_before: Vec::new(),
            moving_bits: BitSet::default(),
            targets: Vec::new(),
            split_bodies: Vec::new(),
            split_contacts: Vec::new(),
            split_joints: Vec::new(),
            split_stack: Vec::new(),
            new_pairs: Vec::new(),
            new_contacts: Vec::new(),
            changes: Vec::new(),
            skipped: Vec::new(),
            extras: Vec::new(),
            overflow_deltas: Vec::new(),
            fast_regular: Vec::new(),
            fast_bullets: Vec::new(),
            continuous_enlarged: PerWorker::new(1, |_| Vec::new()),
            pre_solve_skipped: PerWorker::new(1, |_| Vec::new()),
        }
    }
}
