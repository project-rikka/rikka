mod bodies;
mod characters;
mod draw;
mod origin;
mod queries;
mod record;
mod reset;
mod snapshot;
mod tick;

use std::fmt;
use std::sync::Arc;

use rikka_physics_core::{Executor, Fnv1a, Profile, ProfileEntry};
use rikka_physics_math::{Simd, Vec3};
use rikka_physics_rigid::{BodyId, Character, JointId, RigidWorld};

use crate::call::Call;
use crate::clock::Clock;
use crate::events::FrameEvents;
use crate::history::{History, Scope, Setup};
use crate::interpolation::BodyPoses;
use crate::recording::Recording;
use crate::settings::Settings;
use crate::targets::Targets;
use crate::{CharacterId, Result, WorldBuilder};
use rikka_physics_core::Pool;

pub struct World {
    settings: Settings,
    gravity: Vec3,
    executor: Arc<dyn Executor>,
    rigid: RigidWorld,
    targets: Targets,
    poses: BodyPoses,
    clock: Clock,
    events: FrameEvents,
    bodies: Vec<BodyId>,
    joints: Vec<JointId>,
    characters: Pool<CharacterId, Character>,
    history: History,
    recorder: Option<Recording>,
    replaying: bool,
    tick_hashes: Vec<u64>,
    rebuilding: bool,
    profile: Profile,
}

impl fmt::Debug for World {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("World")
            .field("tick_rate", &self.settings.tick_rate)
            .field("substeps", &self.settings.substeps)
            .field("simd", &self.settings.simd.name())
            .field("workers", &self.executor.workers())
            .field("time", &self.clock.time())
            .field("rigid", &self.rigid)
            .field("recording", &self.recorder.is_some())
            .finish()
    }
}

impl World {
    pub fn builder() -> WorldBuilder {
        WorldBuilder::new()
    }

    pub(crate) fn new(settings: Settings, executor: Arc<dyn Executor>) -> Result<Self> {
        Ok(Self {
            rigid: settings.rigid_world(executor.clone())?,
            gravity: settings.gravity,
            clock: Clock::new(settings.tick_rate),
            settings,
            executor,
            targets: Targets::default(),
            poses: BodyPoses::default(),
            events: FrameEvents::default(),
            bodies: Vec::new(),
            joints: Vec::new(),
            characters: Pool::default(),
            history: History::default(),
            recorder: None,
            replaying: false,
            tick_hashes: Vec::new(),
            rebuilding: false,
            profile: Profile::new(),
        })
    }

    pub fn tick_rate(&self) -> u32 {
        self.settings.tick_rate
    }

    pub fn substeps(&self) -> u32 {
        self.settings.substeps
    }

    pub fn max_ticks_per_frame(&self) -> u32 {
        self.settings.max_ticks
    }

    pub fn simd(&self) -> Simd {
        self.settings.simd
    }

    pub fn workers(&self) -> usize {
        self.executor.workers()
    }

    pub fn ticks(&self) -> i64 {
        self.clock.tick()
    }

    pub fn time(&self) -> f64 {
        self.clock.time()
    }

    pub fn motion_time(&self) -> f64 {
        self.clock.motion_time()
    }

    pub fn interpolation(&self) -> f32 {
        self.clock.alpha()
    }

    pub fn dropped_time(&self) -> f64 {
        self.clock.dropped()
    }

    pub fn set_profiling(&mut self, enabled: bool) {
        self.profile.set_enabled(enabled);
        self.rigid.profile_mut().set_enabled(enabled);
    }

    pub fn profile(&self) -> impl Iterator<Item = ProfileEntry> + '_ {
        self.rigid
            .profile()
            .entries()
            .iter()
            .chain(self.profile.entries())
            .copied()
    }

    pub fn reset_profile(&mut self) {
        self.profile.reset();
        self.rigid.profile_mut().reset();
    }
    pub fn state_hash(&self) -> u64 {
        let mut hasher = Fnv1a::new();
        hasher.u64(self.rigid.state_hash());
        for (id, character) in self.characters.iter() {
            hasher.u32(id.index);
            hasher.vec3(character.position());
            hasher.vec3(character.velocity());
        }
        hasher.finish()
    }

    fn log(&mut self, scope: Scope, call: impl FnOnce() -> Call) {
        if self.rebuilding {
            return;
        }
        let history = self.history.wants(scope);
        if !history && self.recorder.is_none() {
            return;
        }
        let call = call();
        if let Some(recorder) = &mut self.recorder {
            recorder.push_call(call.clone());
        }
        if history {
            self.history.call(call);
        }
    }

    fn log_setup(&mut self, setup: impl FnOnce() -> Setup) {
        if self.rebuilding {
            return;
        }
        let setup = setup();
        if let Some(recorder) = &mut self.recorder {
            recorder.push_setup(setup.clone());
        }
        self.history.setup(setup);
    }
}
