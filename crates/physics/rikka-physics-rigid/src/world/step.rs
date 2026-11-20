use rikka_physics_collision::{CollideInput, Collider, PairFilter, TOUCHING};
use rikka_physics_core::{Block, Stage, Stopwatch};

use super::RigidWorld;
use crate::body::{BodyRecord, NONE};
use crate::contact::{HIT_EVENTS, SKIP};
use crate::joint::JointRecord;
use crate::set::{AWAKE_SET, FIRST_SLEEPING_SET};
use crate::shape_record::ShapeRecord;
use crate::solver::StepParams;
use crate::table::Table;
use crate::{CollisionFilter, ContactHitEvent, JointEvent, JointEventKind, ShapeId};

const PRE_SOLVE_BLOCK: usize = 64;

pub(crate) struct WorldFilter<'a> {
    pub(crate) bodies: &'a Table<BodyRecord>,
    pub(crate) joints: &'a Table<JointRecord>,
    pub(crate) shapes: &'a Table<ShapeRecord>,
    pub(crate) colliders: &'a [Collider],
    pub(crate) custom: Option<&'a dyn CollisionFilter>,
}

impl WorldFilter<'_> {
    fn joint_blocks(&self, a: u32, b: u32) -> bool {
        let (first, other) = if self.bodies.get(a).joint_count <= self.bodies.get(b).joint_count {
            (a, b)
        } else {
            (b, a)
        };
        let mut key = self.bodies.get(first).joint_head;
        while key != NONE {
            let joint = self.joints.get(key >> 1);
            if !joint.collide_connected && (joint.body_a == other || joint.body_b == other) {
                return true;
            }
            key = joint.edges[(key & 1) as usize].next;
        }
        false
    }
}

impl PairFilter for WorldFilter<'_> {
    fn should_collide(&self, collider_a: u32, collider_b: u32) -> bool {
        let a = self.colliders[collider_a as usize].owner;
        let b = self.colliders[collider_b as usize].owner;
        if self.joint_blocks(a, b) {
            return false;
        }
        match self.custom {
            Some(custom) => custom.should_collide(
                ShapeId {
                    index: collider_a,
                    generation: self.shapes.generation(collider_a),
                },
                ShapeId {
                    index: collider_b,
                    generation: self.shapes.generation(collider_b),
                },
            ),
            None => true,
        }
    }
}

impl RigidWorld {
    pub fn step(&mut self, dt: f32) {
        let Some(buffers) = self.prepare_step(dt) else {
            return;
        };
        let range = 0..self.solver.plan.stages.len();
        if self.profile.is_enabled() {
            for stage in range {
                let name = self.solver.plan.stages[stage].name;
                let mut watch = Stopwatch::start(&self.profile);
                self.run_stages(stage..stage + 1, &buffers);
                watch.lap(&mut self.profile, name);
            }
        } else {
            self.run_stages(range, &buffers);
        }
        let mut watch = Stopwatch::start(&self.profile);
        self.after_solve();
        watch.lap(&mut self.profile, "after solve");
    }

    fn prepare_step(&mut self, dt: f32) -> Option<crate::solver::execute::RawBuffers> {
        if !self.detect(dt) {
            return None;
        }
        let mut watch = Stopwatch::start(&self.profile);
        self.build_plan();
        let buffers = self.reserve_buffers();
        watch.lap(&mut self.profile, "build plan");
        Some(buffers)
    }

    fn detect(&mut self, dt: f32) -> bool {
        self.events.clear();
        self.step_index += 1;
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        self.solver.params = StepParams::new(&self.config, dt);
        self.reorder_if_due();
        self.apply_targets(dt);
        let mut watch = Stopwatch::start(&self.profile);
        self.update_pairs();
        watch.lap(&mut self.profile, "update pairs");
        self.collide();
        watch.lap(&mut self.profile, "collide");
        if dt <= 0.0 {
            self.update_sensors();
            return false;
        }
        self.run_pre_solve();
        true
    }

    fn apply_targets(&mut self, dt: f32) {
        let targets = std::mem::take(&mut self.solver.targets);
        for &body in &targets {
            let record = *self.bodies.get(body);
            let Some(target) = record.target else {
                continue;
            };
            if !self.bodies.is_alive(body, self.bodies.generation(body))
                || record.set != AWAKE_SET
                || dt <= 0.0
            {
                continue;
            }
            let awake = self.sets.awake_mut();
            let local = record.local as usize;
            let pose = awake.poses[local];
            let goal = target.apply(pose.local_center);
            let (axis, angle) =
                rikka_physics_math::axis_angle(target.q * pose.transform.q.conjugate());
            let state = &mut awake.states[local];
            state.v = (goal - pose.center) / dt;
            state.w = axis * (angle / dt);
        }
        self.solver.targets = targets;
        self.solver.targets.clear();
    }

    pub fn refresh_broadphase(&mut self) {
        self.update_pairs();
    }

    fn update_pairs(&mut self) {
        let executor = self.executor.clone();
        let filter = WorldFilter {
            bodies: &self.bodies,
            joints: &self.joints,
            shapes: &self.shapes,
            colliders: &self.colliders,
            custom: self.filter.as_deref(),
        };
        let pairs = self.broadphase.update_pairs(&*executor, &filter);
        let mut created = std::mem::take(&mut self.solver.new_pairs);
        created.clear();
        created.extend_from_slice(pairs);
        let mut watch = Stopwatch::start(&self.profile);
        self.create_contacts(&created);
        watch.lap(&mut self.profile, "create contacts");
        self.solver.new_pairs = created;
    }

    fn collide(&mut self) {
        let executor = self.executor.clone();
        let input = CollideInput {
            colliders: &self.colliders,
            frames: &self.frames,
            fat: self.broadphase.fat_bounds(),
            awake: &self.awake_contacts,
            tolerance: self.config.tolerance,
            recycle: self.config.recycling,
        };
        let changes = self.narrowphase.collide(
            &*executor,
            self.simd,
            &input,
            &mut self.headers,
            &mut self.geometry,
        );
        let mut list = std::mem::take(&mut self.solver.changes);
        list.clear();
        list.extend_from_slice(changes);
        let mut watch = Stopwatch::start(&self.profile);
        self.process_contact_changes(&list);
        watch.lap(&mut self.profile, "contact changes");
        self.solver.changes = list;
    }

    fn run_pre_solve(&mut self) {
        for &contact in &self.solver.skipped {
            if self
                .contacts
                .is_alive(contact, self.contacts.generation(contact))
            {
                self.contacts.get_mut(contact).flags &= !SKIP;
            }
        }
        self.solver.skipped.clear();
        let Some(hook) = self.pre_solve.as_deref() else {
            return;
        };
        let executor = self.executor.clone();
        let workers = executor.workers();
        if self.solver.pre_solve_skipped.len() != workers {
            self.solver.pre_solve_skipped =
                rikka_physics_core::PerWorker::new(workers, |_| Vec::new());
        }
        let awake = &self.awake_contacts;
        let found = &self.solver.pre_solve_skipped;
        let task = |block: Block| {
            let mut out = found.local(block.worker);
            for &contact in &awake[block.range(PRE_SOLVE_BLOCK, awake.len())] {
                if self.headers[contact as usize].flags & TOUCHING == 0 {
                    continue;
                }
                let record = self.contacts.get(contact);
                let a = ShapeId {
                    index: record.shape_a,
                    generation: self.shapes.generation(record.shape_a),
                };
                let b = ShapeId {
                    index: record.shape_b,
                    generation: self.shapes.generation(record.shape_b),
                };
                if !hook.pre_solve(a, b, &self.geometry[contact as usize].manifold) {
                    out.push(contact);
                }
            }
        };
        executor.run(
            &[Stage::for_items("pre-solve", awake.len(), PRE_SOLVE_BLOCK)],
            &task,
        );
        for list in self.solver.pre_solve_skipped.iter_mut() {
            self.solver.skipped.append(list);
        }
        self.solver.skipped.sort_unstable();
        for &contact in &self.solver.skipped {
            self.contacts.get_mut(contact).flags |= SKIP;
        }
    }

    pub(crate) fn after_solve(&mut self) {
        let mut watch = Stopwatch::start(&self.profile);
        self.collect_finalize();
        self.solve_continuous();
        watch.lap(&mut self.profile, "continuous");
        self.events
            .body_moves
            .truncate(self.solver.plan.awake_bodies as usize);
        let mut watch = Stopwatch::start(&self.profile);
        for &shape in &self.solver.enlarged {
            self.broadphase.touch(shape);
        }
        let impacts = std::mem::take(&mut self.solver.impact_enlarged);
        for &shape in &impacts {
            let body = self.colliders[shape as usize].owner;
            let transform = self.pose_of(body).transform;
            let (_, fat) = self.fat_bounds(shape, transform);
            self.broadphase.move_proxy(shape, fat);
        }
        self.solver.impact_enlarged = impacts;
        watch.lap(&mut self.profile, "enlarge proxies");
        self.keep_kinematic_neighbours_awake();
        self.report_joints();
        self.report_hits();
        watch.lap(&mut self.profile, "step events");
        if self.config.sleeping {
            self.sleep_islands();
            watch.lap(&mut self.profile, "sleep islands");
        }
        self.update_sensors();
        watch.lap(&mut self.profile, "sensors");
    }

    fn keep_kinematic_neighbours_awake(&mut self) {
        let moving = std::mem::take(&mut self.solver.moving_kinematic);
        let mut before = std::mem::take(&mut self.solver.moving_before);
        self.solver.moving_bits.grow(self.bodies.capacity());
        for &body in &moving {
            if !self.solver.moving_bits.get(body as usize) {
                self.wake_touching(body);
            }
        }
        for &body in &before {
            self.solver.moving_bits.clear(body as usize);
        }
        for &body in &moving {
            self.solver.moving_bits.set(body as usize);
        }
        before.clear();
        self.solver.moving_kinematic = before;
        self.solver.moving_before = moving;
    }

    fn wake_touching(&mut self, body: u32) {
        let mut key = self.bodies.get(body).contact_head;
        while key != NONE {
            let contact = self.contacts.get(key >> 1);
            key = contact.edges[(key & 1) as usize].next;
            self.keep_island_awake(contact.island);
        }
        let mut key = self.bodies.get(body).joint_head;
        while key != NONE {
            let joint = self.joints.get(key >> 1);
            key = joint.edges[(key & 1) as usize].next;
            self.keep_island_awake(joint.island);
        }
    }

    fn keep_island_awake(&mut self, island: u32) {
        if island == NONE {
            return;
        }
        let set = self.islands.get(island).set;
        if set >= FIRST_SLEEPING_SET {
            self.wake_set(set);
        }
        self.solver.awake_islands.set_growing(island as usize);
    }

    fn touches_moving_kinematic(&self, island: u32) -> bool {
        let moving = |body: u32| self.solver.moving_bits.get(body as usize);
        let record = self.islands.get(island);
        let mut contact = record.contacts.head;
        while contact != NONE {
            let c = self.contacts.get(contact);
            if moving(c.body_a()) || moving(c.body_b()) {
                return true;
            }
            contact = c.island_next;
        }
        let mut joint = record.joints.head;
        while joint != NONE {
            let j = self.joints.get(joint);
            if moving(j.body_a) || moving(j.body_b) {
                return true;
            }
            joint = j.island_next;
        }
        false
    }

    fn report_joints(&mut self) {
        if self.monitored_joints == 0 {
            return;
        }
        let mut broken = std::mem::take(&mut self.solver.split_stack);
        broken.clear();
        for index in 0..self.joints.capacity() as u32 {
            if !self.joints.is_alive(index, self.joints.generation(index)) {
                continue;
            }
            let joint = self.joints.get_mut(index);
            if joint.set != AWAKE_SET {
                continue;
            }
            let (force, torque) = joint.reaction();
            let breaks = force > joint.break_force || torque > joint.break_torque;
            let over = force >= joint.force_threshold || torque >= joint.torque_threshold;
            let report = over && !joint.over_threshold;
            joint.over_threshold = over;
            let user_data = joint.user_data;
            if breaks {
                broken.push(index);
            }
            if breaks || report {
                self.events.joints.push(JointEvent {
                    joint: self.joint_id(index),
                    kind: if breaks {
                        JointEventKind::Broken
                    } else {
                        JointEventKind::Threshold
                    },
                    force,
                    torque,
                    user_data,
                });
            }
        }
        for &joint in &broken {
            self.break_joint(joint);
        }
        self.solver.split_stack = broken;
    }

    fn report_hits(&mut self) {
        if self.hit_contacts == 0 {
            return;
        }
        let threshold = self.config.hit_event_threshold * self.config.tolerance.length_scale;
        for &contact in &self.awake_contacts {
            let record = self.contacts.get(contact);
            if record.flags & HIT_EVENTS == 0 || record.color == NONE {
                continue;
            }
            let manifold = &self.geometry[contact as usize].manifold;
            let Some(point) = manifold
                .points()
                .iter()
                .filter(|point| point.total_normal_impulse > 0.0)
                .min_by(|a, b| a.normal_velocity.total_cmp(&b.normal_velocity))
            else {
                continue;
            };
            let approach = -point.normal_velocity;
            if approach > threshold {
                self.events.contact_hit.push(ContactHitEvent {
                    shape_a: ShapeId {
                        index: record.shape_a,
                        generation: self.shapes.generation(record.shape_a),
                    },
                    shape_b: ShapeId {
                        index: record.shape_b,
                        generation: self.shapes.generation(record.shape_b),
                    },
                    point: point.point,
                    normal: manifold.normal,
                    approach_speed: approach,
                    material_a: manifold.material.user_a,
                    material_b: manifold.material.user_b,
                });
            }
        }
    }

    fn sleep_islands(&mut self) {
        let mut candidates = std::mem::take(&mut self.solver.split_bodies);
        candidates.clear();
        candidates.extend(
            self.sets
                .awake()
                .islands
                .iter()
                .copied()
                .filter(|&island| !self.solver.awake_islands.get(island as usize)),
        );
        candidates.sort_unstable();
        let mut split = None;
        for &island in &candidates {
            let record = self.islands.get(island);
            if !record.alive || record.set != AWAKE_SET {
                continue;
            }
            if !self.solver.moving_before.is_empty() && self.touches_moving_kinematic(island) {
                continue;
            }
            if record.constraint_removed > 0 && record.bodies.count > 1 {
                split.get_or_insert(island);
                continue;
            }
            self.try_sleep_island(island);
        }
        self.solver.split_bodies = candidates;
        if let Some(island) = split {
            self.split_island(island);
        }
    }
}
