use std::ops::Range;

use rikka_physics_core::Stage;

use crate::joint::JointKind;
use crate::solver::contact::ContactPass;

const BODY_BLOCK: (usize, usize) = (32, 256);
const PREPARE_BLOCK: (usize, usize) = (4, 32);
const COLOR_BLOCK: (usize, usize) = (2, 16);
const BLOCKS_PER_WORKER: usize = 4;
pub(crate) const OVERFLOW_BLOCK: usize = 16;

fn width(items: usize, workers: usize, (low, high): (usize, usize)) -> usize {
    (items / (BLOCKS_PER_WORKER * workers)).clamp(low, high)
}
pub(crate) const SMALL_COLOR: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct JointBatch {
    pub kind: JointKind,
    pub slot: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TaskStep {
    Contacts(u32),
    Joints(u32),
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ColorPlan {
    pub contacts: Range<u32>,
    pub joints: Range<u32>,
}

impl ColorPlan {
    pub(crate) fn batches(&self) -> u32 {
        self.contacts.len() as u32 + self.joints.len() as u32
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TaskPlan {
    pub bodies: Range<u32>,
    pub sequence: Range<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StageOp {
    Prepare,
    IslandTasks,
    IntegrateVelocities,
    Colors {
        first: u32,
        count: u32,
        pass: ContactPass,
        width: u32,
    },
    OverflowSolve {
        pass: ContactPass,
    },
    OverflowApply,
    IntegratePositions,
    Finalize,
}

#[derive(Debug, Default)]
pub(crate) struct Plan {
    pub contact_ids: Vec<u32>,
    pub contact_counts: Vec<u8>,
    pub contact_batches: u32,
    pub colored_contacts: u32,
    pub colored_joints: u32,
    pub joint_ids: Vec<u32>,
    pub joint_counts: Vec<u8>,
    pub joint_batches: Vec<JointBatch>,
    pub joint_slots: [u32; JointKind::COUNT],
    pub colors: Vec<ColorPlan>,
    pub overflow: ColorPlan,
    pub overflow_bodies: Vec<u32>,
    pub overflow_offsets: Vec<u32>,
    pub overflow_entries: Vec<u32>,
    pub overflow_split: Vec<f32>,
    pub tasks: Vec<TaskPlan>,
    pub task_bodies: Vec<u32>,
    pub task_sequence: Vec<TaskStep>,
    pub colored_bodies: Vec<u32>,
    pub awake_bodies: u32,
    pub stages: Vec<Stage>,
    pub ops: Vec<StageOp>,
    pub lanes: usize,
    pub workers: usize,
    pub body_width: usize,
    pub prepare_width: usize,
}

impl Plan {
    pub(crate) fn clear(&mut self, lanes: usize) {
        self.contact_ids.clear();
        self.contact_counts.clear();
        self.contact_batches = 0;
        self.colored_contacts = 0;
        self.colored_joints = 0;
        self.joint_ids.clear();
        self.joint_counts.clear();
        self.joint_batches.clear();
        self.joint_slots = [0; JointKind::COUNT];
        self.colors.clear();
        self.overflow = ColorPlan::default();
        self.tasks.clear();
        self.overflow_bodies.clear();
        self.overflow_offsets.clear();
        self.overflow_entries.clear();
        self.task_bodies.clear();
        self.task_sequence.clear();
        self.colored_bodies.clear();
        self.stages.clear();
        self.ops.clear();
        self.lanes = lanes;
    }

    pub(crate) fn keep_colored(&mut self) {
        self.contact_batches = self.colored_contacts;
        self.contact_ids
            .truncate(self.colored_contacts as usize * self.lanes);
        self.contact_counts.truncate(self.colored_contacts as usize);
        self.joint_batches.truncate(self.colored_joints as usize);
        self.joint_counts.truncate(self.colored_joints as usize);
        self.joint_ids
            .truncate(self.colored_joints as usize * self.lanes);
        self.joint_slots = [0; JointKind::COUNT];
        for batch in &self.joint_batches {
            self.joint_slots[batch.kind.index()] += 1;
        }
        self.tasks.clear();
        self.overflow_bodies.clear();
        self.overflow_offsets.clear();
        self.overflow_entries.clear();
        self.task_bodies.clear();
        self.task_sequence.clear();
        self.colored_bodies.clear();
        self.stages.clear();
        self.ops.clear();
    }

    pub(crate) fn push_contact_batch(&mut self, ids: &[u32]) -> u32 {
        debug_assert!(!ids.is_empty() && ids.len() <= self.lanes);
        let batch = self.contact_batches;
        self.contact_ids.extend_from_slice(ids);
        self.contact_counts.push(ids.len() as u8);
        self.contact_ids.extend(std::iter::repeat_n(
            super::contact::NO_CONTACT,
            self.lanes - ids.len(),
        ));
        self.contact_batches += 1;
        batch
    }

    pub(crate) fn push_joint_batch(&mut self, kind: JointKind, ids: &[u32]) -> u32 {
        debug_assert!(!ids.is_empty() && ids.len() <= self.lanes);
        let batch = self.joint_batches.len() as u32;
        self.joint_ids.extend_from_slice(ids);
        self.joint_counts.push(ids.len() as u8);
        self.joint_ids.extend(std::iter::repeat_n(
            crate::joint::NO_JOINT,
            self.lanes - ids.len(),
        ));
        let slot = &mut self.joint_slots[kind.index()];
        self.joint_batches.push(JointBatch { kind, slot: *slot });
        *slot += 1;
        batch
    }

    pub(crate) fn contact_batch_ids(&self, batch: u32) -> &[u32] {
        let start = batch as usize * self.lanes;
        &self.contact_ids[start..start + self.contact_counts[batch as usize] as usize]
    }

    pub(crate) fn joint_batch_ids(&self, batch: u32) -> &[u32] {
        let start = batch as usize * self.lanes;
        &self.joint_ids[start..start + self.joint_counts[batch as usize] as usize]
    }

    fn push_stage(&mut self, name: &'static str, blocks: u32, op: StageOp) {
        if blocks > 0 {
            self.stages.push(Stage::new(name, blocks));
            self.ops.push(op);
        }
    }

    fn push_colors(&mut self, pass: ContactPass) {
        let name = match pass {
            ContactPass::Warm => "warm start color",
            ContactPass::Solve => "solve color",
            ContactPass::Relax => "relax color",
            ContactPass::RelaxRestitution => "relax and restitution color",
        };
        let active = self.colors.len() as u32;
        let mut merged_from = active;
        while merged_from > 0 && self.colors[merged_from as usize - 1].batches() <= SMALL_COLOR {
            merged_from -= 1;
        }
        if active - merged_from < 2 {
            merged_from = active;
        }
        for color in 0..merged_from {
            let batches = self.colors[color as usize].batches() as usize;
            let width = width(batches, self.workers, COLOR_BLOCK);
            self.push_stage(
                name,
                batches.div_ceil(width) as u32,
                StageOp::Colors {
                    first: color,
                    count: 1,
                    pass,
                    width: width as u32,
                },
            );
        }
        if merged_from < active {
            self.push_stage(
                "solve small colors",
                1,
                StageOp::Colors {
                    first: merged_from,
                    count: active - merged_from,
                    pass,
                    width: 0,
                },
            );
        }
        let overflow = self.overflow.batches() as usize;
        if overflow > 0 {
            self.push_stage(
                "solve overflow",
                overflow.div_ceil(OVERFLOW_BLOCK) as u32,
                StageOp::OverflowSolve { pass },
            );
            self.push_stage(
                "apply overflow",
                self.overflow_bodies.len().div_ceil(OVERFLOW_BLOCK) as u32,
                StageOp::OverflowApply,
            );
        }
    }

    pub(crate) fn build_stages(&mut self, substeps: u32, workers: usize) {
        self.workers = workers;
        let prepare_batches = self.colored_contacts as usize + self.colored_joints as usize;
        self.prepare_width = width(prepare_batches, workers, PREPARE_BLOCK);
        self.body_width = width(self.awake_bodies as usize, workers, BODY_BLOCK);
        self.push_stage(
            "prepare constraints",
            prepare_batches.div_ceil(self.prepare_width) as u32,
            StageOp::Prepare,
        );
        self.push_stage(
            "island tasks",
            self.tasks.len() as u32,
            StageOp::IslandTasks,
        );
        let body_blocks = self.colored_bodies.len().div_ceil(self.body_width) as u32;
        for substep in 0..substeps {
            self.push_stage(
                "integrate velocities",
                body_blocks,
                StageOp::IntegrateVelocities,
            );
            self.push_colors(ContactPass::Warm);
            self.push_colors(ContactPass::Solve);
            self.push_stage(
                "integrate positions",
                body_blocks,
                StageOp::IntegratePositions,
            );
            let pass = if substep + 1 == substeps {
                ContactPass::RelaxRestitution
            } else {
                ContactPass::Relax
            };
            self.push_colors(pass);
        }
        self.push_stage(
            "finalize bodies",
            (self.awake_bodies as usize).div_ceil(self.body_width) as u32,
            StageOp::Finalize,
        );
    }
}
