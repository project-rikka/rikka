use rikka_physics_collision::{
    BodyFrame, Collider, PairFilter, ProxyKind, SweptShape, shape_time_of_impact,
};
use rikka_physics_core::{Block, PerWorker, Shared, Stage};
use rikka_physics_geometry::{ShapeGeometry, Sweep};
use rikka_physics_math::{Vec3, nlerp};

use crate::BodyMoveEvent;
use crate::body::{BULLET, BodyRecord, NONE};
use crate::set::{AWAKE_SET, BodyPose};
use crate::shape_record::ShapeRecord;
use crate::solver::state::{BodyState, LOCK_LINEAR_X, LOCK_LINEAR_Y, LOCK_LINEAR_Z};
use crate::table::Table;
use crate::world::{RigidWorld, WorldFilter};

const BLOCK: (usize, usize) = (4, 32);
const BLOCKS_PER_WORKER: usize = 4;

struct Continuous<'a> {
    bodies: &'a Table<BodyRecord>,
    shapes: &'a [ShapeRecord],
    colliders: &'a [Collider],
    broadphase: &'a rikka_physics_collision::Broadphase,
    filter: &'a WorldFilter<'a>,
    poses: Shared<'a, BodyPose>,
    states: Shared<'a, BodyState>,
    frames: Shared<'a, BodyFrame>,
    moves: Shared<'a, BodyMoveEvent>,
    enlarged: &'a PerWorker<Vec<u32>>,
    gravity: Vec3,
    dt: f32,
    tolerance: rikka_physics_geometry::Tolerance,
}

impl Continuous<'_> {
    fn solve(&self, body: u32, worker: u32) {
        let record = self.bodies.get(body);
        let local = record.local as usize;
        let pose = unsafe { self.poses.get(local) };
        let bullet = record.flags & BULLET != 0;
        let sweep = Sweep {
            local_center: pose.local_center,
            c1: pose.center0,
            c2: pose.center,
            q1: pose.rotation0,
            q2: pose.transform.q,
        };
        let kinds: &[ProxyKind] = if bullet {
            &[ProxyKind::Static, ProxyKind::Kinematic, ProxyKind::Dynamic]
        } else {
            &[ProxyKind::Static]
        };
        let mut fraction = 1.0f32;
        let mut shape = record.shape_head;
        while shape != NONE {
            let collider = &self.colliders[shape as usize];
            let next = self.shapes[shape as usize].next;
            if !collider.responds() || collider.shape.as_convex().is_none() {
                shape = next;
                continue;
            }
            let swept = SweptShape::new(&collider.shape, sweep, fraction);
            let bounds = swept.bounds;
            for &kind in kinds {
                self.broadphase.query_kind(kind, &bounds, |other| {
                    let target = &self.colliders[other as usize];
                    if target.owner == body
                        || !target.responds()
                        || !collider.filter.should_collide(&target.filter)
                    {
                        return true;
                    }
                    let owner = self.bodies.get(target.owner);
                    if owner.flags & BULLET != 0
                        || !self
                            .filter
                            .should_collide(shape.min(other), shape.max(other))
                    {
                        return true;
                    }
                    let frame = unsafe { &*self.frames.as_ptr().add(target.owner as usize) };
                    if let Some(hit) = shape_time_of_impact(
                        &target.shape,
                        frame.transform,
                        &swept,
                        fraction,
                        self.tolerance,
                    ) {
                        fraction = hit;
                    }
                    true
                });
            }
            shape = next;
        }
        if fraction >= 1.0 {
            pose.center0 = pose.center;
            pose.rotation0 = pose.transform.q;
            return;
        }
        let rotation = nlerp(sweep.q1, sweep.q2, fraction);
        let center = sweep.c1.lerp(sweep.c2, fraction);
        pose.set_center(center, rotation);
        pose.center0 = center;
        pose.rotation0 = rotation;
        let state = unsafe { self.states.get(local) };
        let mut loss = self.gravity * (-(1.0 - fraction) * self.dt);
        for (axis, lock) in [LOCK_LINEAR_X, LOCK_LINEAR_Y, LOCK_LINEAR_Z]
            .into_iter()
            .enumerate()
        {
            if state.flags & lock != 0 {
                loss[axis] = 0.0;
            }
        }
        state.v += loss;
        let frame = unsafe { self.frames.get(body as usize) };
        frame.transform = pose.transform;
        frame.center = pose.center;
        unsafe { self.moves.get(local) }.transform = pose.transform;
        let mut enlarged = self.enlarged.local(worker);
        let mut shape = record.shape_head;
        while shape != NONE {
            let bounds = frame.swept(self.colliders[shape as usize].shape.aabb(pose.transform));
            if !self.broadphase.fat(shape).contains(&bounds) {
                enlarged.push(shape);
            }
            shape = self.shapes[shape as usize].next;
        }
    }
}

impl RigidWorld {
    pub(crate) fn solve_continuous(&mut self) {
        self.solver.impact_enlarged.clear();
        let fast = [&self.solver.fast_regular, &self.solver.fast_bullets];
        if fast.iter().all(|list| list.is_empty()) {
            return;
        }
        if !self.config.continuous {
            for body in fast.into_iter().flatten() {
                let record = self.bodies.get(*body);
                let pose = &mut self.sets.sets[AWAKE_SET as usize].poses[record.local as usize];
                pose.center0 = pose.center;
                pose.rotation0 = pose.transform.q;
            }
            return;
        }
        let regular = std::mem::take(&mut self.solver.fast_regular);
        let bullets = std::mem::take(&mut self.solver.fast_bullets);
        let executor = self.executor.clone();
        if self.solver.continuous_enlarged.len() != executor.workers() {
            self.solver.continuous_enlarged = PerWorker::new(executor.workers(), |_| Vec::new());
        }
        let filter = WorldFilter {
            bodies: &self.bodies,
            joints: &self.joints,
            shapes: &self.shapes,
            colliders: &self.colliders,
            custom: self.filter.as_deref(),
        };
        let awake = &mut self.sets.sets[AWAKE_SET as usize];
        let context = Continuous {
            bodies: &self.bodies,
            shapes: &self.shapes.records,
            colliders: &self.colliders,
            broadphase: &self.broadphase,
            filter: &filter,
            poses: Shared::new(&mut awake.poses),
            states: Shared::new(&mut awake.states),
            frames: Shared::new(&mut self.frames),
            moves: Shared::new(&mut self.events.body_moves),
            enlarged: &self.solver.continuous_enlarged,
            gravity: self.config.gravity,
            dt: self.solver.params.dt,
            tolerance: self.config.tolerance,
        };
        let widths = [regular.len(), bullets.len()]
            .map(|len| (len / (BLOCKS_PER_WORKER * executor.workers())).clamp(BLOCK.0, BLOCK.1));
        let stages = [
            Stage::for_items("continuous", regular.len(), widths[0]),
            Stage::for_items("bullets", bullets.len(), widths[1]),
        ];
        let lists = [&regular, &bullets];
        let task = |block: Block| {
            let list = lists[block.stage as usize];
            for &body in &list[block.range(widths[block.stage as usize], list.len())] {
                context.solve(body, block.worker);
            }
        };
        executor.run(&stages, &task);
        for list in self.solver.continuous_enlarged.iter_mut() {
            self.solver.impact_enlarged.append(list);
        }
        for list in self.solver.continuous_enlarged.iter_mut() {
            list.reserve(self.solver.impact_enlarged.len());
        }
        self.solver.fast_regular = regular;
        self.solver.fast_bullets = bullets;
    }
}
