mod bodies;
mod contacts;
mod create;
mod islands;
mod joints;
mod mover;
mod origin;
mod queries;
mod reorder;
mod sensors;
mod shapes;
mod sleep;
mod step;

pub use contacts::ContactData;
pub use queries::{ClosestPoint, RayHit};
pub(crate) use step::WorldFilter;

use std::sync::Arc;

use rikka_physics_collision::{
    BodyFrame, Broadphase, Collider, ContactGeometry, ContactHeader, Narrowphase, Sensors,
};
use rikka_physics_core::{Executor, Fnv1a, Profile};
use rikka_physics_math::Simd;

use crate::body::BodyRecord;
use crate::config::Config;
use crate::contact::ContactRecord;
use crate::events::Events;
use crate::graph::ConstraintGraph;
use crate::island::Islands;
use crate::joint::JointRecord;
use crate::set::SolverSets;
use crate::shape_record::ShapeRecord;
use crate::solver::Solver;
use crate::table::Table;
use crate::{CollisionFilter, PreSolve, RigidWorldBuilder};

pub struct RigidWorld {
    pub(crate) config: Config,
    pub(crate) simd: Simd,
    pub(crate) executor: Arc<dyn Executor>,
    pub(crate) bodies: Table<BodyRecord>,
    pub(crate) frames: Vec<BodyFrame>,
    pub(crate) shapes: Table<ShapeRecord>,
    pub(crate) colliders: Vec<Collider>,
    pub(crate) contacts: Table<ContactRecord>,
    pub(crate) headers: Vec<ContactHeader>,
    pub(crate) geometry: Vec<ContactGeometry>,
    pub(crate) awake_contacts: Vec<u32>,
    pub(crate) joints: Table<JointRecord>,
    pub(crate) sets: SolverSets,
    pub(crate) graph: ConstraintGraph,
    pub(crate) islands: Islands,
    pub(crate) broadphase: Broadphase,
    pub(crate) narrowphase: Narrowphase,
    pub(crate) sensors: Sensors,
    pub(crate) solver: Solver,
    pub(crate) events: Events,
    pub(crate) pre_solve: Option<Box<dyn PreSolve>>,
    pub(crate) filter: Option<Box<dyn CollisionFilter>>,
    pub(crate) step_index: u64,
    pub(crate) hit_contacts: u32,
    pub(crate) monitored_joints: u32,
    pub(crate) profile: Profile,
}

impl std::fmt::Debug for RigidWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RigidWorld")
            .field("simd", &self.simd.name())
            .field("workers", &self.executor.workers())
            .field("bodies", &self.bodies.len())
            .field("shapes", &self.shapes.len())
            .field("contacts", &self.contacts.len())
            .field("joints", &self.joints.len())
            .field("step", &self.step_index)
            .finish()
    }
}

impl RigidWorld {
    pub fn builder() -> RigidWorldBuilder {
        RigidWorldBuilder::new()
    }

    pub(crate) fn new(config: Config, simd: Simd, executor: Arc<dyn Executor>) -> Self {
        Self {
            config,
            simd,
            executor,
            bodies: Table::default(),
            frames: Vec::new(),
            shapes: Table::default(),
            colliders: Vec::new(),
            contacts: Table::default(),
            headers: Vec::new(),
            geometry: Vec::new(),
            awake_contacts: Vec::new(),
            joints: Table::default(),
            sets: SolverSets::default(),
            graph: ConstraintGraph::default(),
            islands: Islands::default(),
            broadphase: Broadphase::new(),
            narrowphase: Narrowphase::new(),
            sensors: Sensors::default(),
            solver: Solver::default(),
            events: Events::default(),
            pre_solve: None,
            filter: None,
            step_index: 0,
            hit_contacts: 0,
            monitored_joints: 0,
            profile: Profile::new(),
        }
    }

    pub fn executor(&self) -> &Arc<dyn Executor> {
        &self.executor
    }

    pub fn simd(&self) -> Simd {
        self.simd
    }

    pub fn substeps(&self) -> u32 {
        self.config.substeps
    }

    pub fn set_pre_solve(&mut self, hook: Option<Box<dyn PreSolve>>) {
        self.pre_solve = hook;
    }

    pub fn set_collision_filter(&mut self, filter: Option<Box<dyn CollisionFilter>>) {
        self.filter = filter;
    }

    pub fn set_gravity(&mut self, gravity: rikka_physics_math::Vec3) {
        self.config.gravity = gravity;
    }

    pub fn gravity(&self) -> rikka_physics_math::Vec3 {
        self.config.gravity
    }

    pub fn tolerance(&self) -> rikka_physics_geometry::Tolerance {
        self.config.tolerance
    }

    pub fn contact_softness(&self) -> crate::Spring {
        self.config.contact_softness
    }

    pub fn joint_softness(&self) -> crate::Spring {
        self.config.joint_softness
    }

    pub fn contact_push_speed(&self) -> f32 {
        self.config.contact_push_speed
    }

    pub fn restitution_threshold(&self) -> f32 {
        self.config.restitution_threshold
    }

    pub fn max_linear_speed(&self) -> f32 {
        self.config.max_linear_speed
    }

    pub fn body_count(&self) -> usize {
        self.bodies.len()
    }

    pub fn contact_count(&self) -> usize {
        self.contacts.len()
    }

    pub fn awake_body_count(&self) -> usize {
        self.sets.awake().bodies.len()
    }

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    pub fn profile_mut(&mut self) -> &mut Profile {
        &mut self.profile
    }

    pub fn step_index(&self) -> u64 {
        self.step_index
    }

    pub fn contact_begin_events(&self) -> &[crate::ContactBeginEvent] {
        &self.events.contact_begin
    }

    pub fn contact_end_events(&self) -> &[crate::ContactEndEvent] {
        &self.events.contact_end
    }

    pub fn contact_hit_events(&self) -> &[crate::ContactHitEvent] {
        &self.events.contact_hit
    }

    pub fn body_move_events(&self) -> &[crate::BodyMoveEvent] {
        &self.events.body_moves
    }

    pub fn joint_events(&self) -> &[crate::JointEvent] {
        &self.events.joints
    }
}

impl RigidWorld {
    pub fn state_hash(&self) -> u64 {
        let mut hash = Fnv1a::new();
        for body in self.bodies.live() {
            let record = self.bodies.get(body);
            let set = self.sets.get(record.set);
            let pose = &set.poses[record.local as usize];
            let state = &set.states[record.local as usize];
            hash.u32(body);
            hash.vec3(pose.transform.p);
            for value in pose.transform.q.to_array() {
                hash.f32(value);
            }
            hash.vec3(state.v);
            hash.vec3(state.w);
        }
        hash.finish()
    }
}
