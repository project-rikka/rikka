use rikka_physics_core::{BitSet, Stopwatch};

use crate::body::NONE;
use crate::graph::{COLOR_COUNT, OVERFLOW};
use crate::joint::JointKind;
use crate::solver::contact::contact_of;
use crate::solver::gather::ColorGather;
use crate::solver::plan::{ColorPlan, Plan};
use crate::world::RigidWorld;

const TASK_LIMIT: u32 = 4096;
const TASK_SHARE: u32 = 16;
const SMALL_ISLAND: u32 = 256;
const SMALL_SCENE: u32 = 2048;

#[derive(Debug, Default)]
pub(crate) struct BuildScratch {
    task_of: Vec<u32>,
    gather: ColorGather,
    pending: Vec<u32>,
    ids: Vec<u32>,
    task_islands: Vec<u32>,
    pairs: Vec<(u32, u32)>,
    colored: Vec<u32>,
    colored_now: Vec<u32>,
    task_bodies: BitSet,
    version: u64,
    valid: bool,
    volatile: bool,
}

impl RigidWorld {
    pub(crate) fn build_plan(&mut self) {
        let mut plan = std::mem::take(&mut self.solver.plan);
        let mut scratch = std::mem::take(&mut self.solver.build);
        let mut watch = Stopwatch::start(&self.profile);
        let awake = self.sets.awake();

        if scratch.task_of.len() < self.islands.records.len() {
            scratch.task_of.resize(self.islands.records.len(), NONE);
        }
        let total: u32 = awake
            .islands
            .iter()
            .map(|&island| {
                let record = self.islands.get(island);
                record.contacts.count + record.joints.count
            })
            .sum();
        scratch.task_islands.clear();
        scratch.colored_now.clear();
        for &island in &awake.islands {
            let record = self.islands.get(island);
            let count = record.contacts.count + record.joints.count;
            let small = count <= SMALL_ISLAND
                || total <= SMALL_SCENE
                || (count <= TASK_LIMIT && count * TASK_SHARE <= total);
            scratch.task_of[island as usize] = if small {
                scratch.task_islands.push(island);
                0
            } else {
                scratch.colored_now.push(island);
                NONE
            };
        }
        let lanes = self.simd.lanes();
        let reuse = scratch.valid
            && plan.lanes == lanes
            && scratch.version == self.graph.version
            && !scratch.volatile
            && self.pre_solve.is_none()
            && scratch.colored == scratch.colored_now;
        if reuse {
            plan.keep_colored();
        } else {
            plan.clear(lanes);
        }
        plan.awake_bodies = awake.bodies.len() as u32;

        watch.lap(&mut self.profile, "plan islands");
        if !reuse {
            scratch.volatile = self.gather_colors(&mut scratch.gather, &scratch.task_of);
            let lanes = plan.lanes;
            for color in 0..COLOR_COUNT {
                let by_count = &scratch.gather.by_color[color * 4..color * 4 + 4];
                let joints = &mut scratch.gather.joints_by_color[color];
                joints.sort_unstable();
                if color as u32 == OVERFLOW {
                    let contact_start = plan.contact_batches;
                    for list in by_count {
                        for &contact in list {
                            plan.push_contact_batch(&[contact]);
                        }
                    }
                    let joint_start = plan.joint_batches.len() as u32;
                    for &(kind, _, joint) in joints.iter() {
                        plan.push_joint_batch(JointKind::ALL[kind as usize], &[joint]);
                    }
                    plan.overflow = ColorPlan {
                        contacts: contact_start..plan.contact_batches,
                        joints: joint_start..plan.joint_batches.len() as u32,
                    };
                    continue;
                }
                let contact_start = plan.contact_batches;
                scratch.pending.clear();
                for list in by_count {
                    scratch.pending.extend_from_slice(list);
                }
                for chunk in scratch.pending.chunks(lanes) {
                    plan.push_contact_batch(chunk);
                }
                let joint_start = plan.joint_batches.len() as u32;
                let mut index = 0;
                while index < joints.len() {
                    let (kind, layout, _) = joints[index];
                    let mut end = index;
                    while end < joints.len()
                        && end - index < lanes
                        && joints[end].0 == kind
                        && joints[end].1 == layout
                    {
                        end += 1;
                    }
                    scratch.ids.clear();
                    scratch
                        .ids
                        .extend(joints[index..end].iter().map(|&(_, _, id)| id));
                    plan.push_joint_batch(JointKind::ALL[kind as usize], &scratch.ids);
                    index = end;
                }
                let color_plan = ColorPlan {
                    contacts: contact_start..plan.contact_batches,
                    joints: joint_start..plan.joint_batches.len() as u32,
                };
                if color_plan.batches() > 0 {
                    plan.colors.push(color_plan);
                }
            }
            plan.colored_contacts = plan.contact_batches;
            plan.colored_joints = plan.joint_batches.len() as u32;
            scratch.version = self.graph.version;
            scratch.valid = true;
            std::mem::swap(&mut scratch.colored, &mut scratch.colored_now);
        }
        watch.lap(&mut self.profile, "plan colors");
        self.build_overflow(&mut plan, &mut scratch.pairs);
        self.schedule_islands(&scratch.task_islands, lanes);
        self.schedule_groups(&scratch.task_islands, &mut plan);
        let awake_bodies = plan.awake_bodies as usize;
        if plan.task_bodies.is_empty() {
            plan.colored_bodies.extend(1..=awake_bodies as u32);
        } else {
            scratch.task_bodies.clear_all();
            scratch.task_bodies.grow(awake_bodies + 1);
            for &body in &plan.task_bodies {
                scratch.task_bodies.set(body as usize);
            }
            plan.colored_bodies.extend(
                (1..=awake_bodies as u32).filter(|&body| !scratch.task_bodies.get(body as usize)),
            );
        }
        plan.build_stages(self.config.substeps, self.executor.workers());
        watch.lap(&mut self.profile, "plan tasks");
        self.solver.plan = plan;
        self.solver.build = scratch;
    }
}

impl RigidWorld {
    fn build_overflow(&self, plan: &mut Plan, pairs: &mut Vec<(u32, u32)>) {
        pairs.clear();
        let awake = self.sets.awake();
        let mut push = |local: u32, a: u32, b: u32| {
            for (side, index) in [(0, a), (1, b)] {
                if index != 0 && awake.motion[index as usize - 1].inv_mass > 0.0 {
                    pairs.push((index, local << 1 | side));
                }
            }
        };
        let contacts = plan.overflow.contacts.clone();
        let joints = plan.overflow.joints.clone();
        for (local, batch) in contacts.clone().enumerate() {
            let contact = contact_of(plan.contact_batch_ids(batch)[0]);
            let record = self.contacts.get(contact);
            push(local as u32, record.solver_a, record.solver_b);
        }
        let base = contacts.len() as u32;
        for (local, batch) in joints.enumerate() {
            let joint = plan.joint_batch_ids(batch)[0];
            let record = self.joints.get(joint);
            push(base + local as u32, record.solver_a, record.solver_b);
        }
        if pairs.is_empty() {
            return;
        }
        pairs.sort_unstable();
        plan.overflow_split.clear();
        plan.overflow_split.resize(awake.bodies.len(), 1.0);
        let mut index = 0;
        while index < pairs.len() {
            let body = pairs[index].0;
            let mut end = index;
            while end < pairs.len() && pairs[end].0 == body {
                end += 1;
            }
            plan.overflow_offsets
                .push(plan.overflow_entries.len() as u32);
            plan.overflow_bodies.push(body);
            plan.overflow_entries
                .extend(pairs[index..end].iter().map(|&(_, entry)| entry));
            plan.overflow_split[body as usize - 1] = (end - index) as f32;
            index = end;
        }
        plan.overflow_offsets
            .push(plan.overflow_entries.len() as u32);
    }
}
