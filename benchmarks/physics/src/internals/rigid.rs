use rikka_physics_rigid::RigidWorld;

use super::{Workload, WorkloadResult};

const TIME_STEP: f32 = 1.0 / 60.0;
const SUBSTEPS: u32 = 4;

type Stepper = Box<dyn FnMut(&mut RigidWorld, u32) -> rikka_physics_rigid::Result<()>>;

struct RigidScene {
    world: RigidWorld,
    stepper: Stepper,
    steps: u32,
}

impl Workload for RigidScene {
    fn step(&mut self) {
        if let Err(error) = (self.stepper)(&mut self.world, self.steps) {
            panic!("scene step {} failed: {error}", self.steps);
        }
        self.world.step(TIME_STEP);
        self.steps += 1;
    }

    fn checksum(&self) -> u64 {
        self.world.state_hash()
    }

    fn enable_profile(&mut self) {
        self.world.profile_mut().set_enabled(true);
    }

    fn profile(&self) -> Vec<(&'static str, std::time::Duration, u32)> {
        self.world
            .profile()
            .entries()
            .iter()
            .map(|entry| (entry.name, entry.duration, entry.count))
            .collect()
    }
}

fn world(threads: usize, sleeping: bool) -> rikka_physics_rigid::Result<RigidWorld> {
    RigidWorld::builder()
        .workers(threads)
        .substeps(SUBSTEPS)
        .sleeping(sleeping)
        .build()
}

pub(crate) fn stepped_scene<S>(
    threads: usize,
    sleeping: bool,
    populate: impl FnOnce(&mut RigidWorld) -> rikka_physics_rigid::Result<S>,
    stepper: impl FnMut(&mut S, &mut RigidWorld, u32) -> rikka_physics_rigid::Result<()> + 'static,
) -> WorkloadResult
where
    S: 'static,
{
    let mut world = world(threads, sleeping)?;
    let mut state = populate(&mut world)?;
    let mut stepper = stepper;
    Ok(Box::new(RigidScene {
        world,
        stepper: Box::new(move |world, step| stepper(&mut state, world, step)),
        steps: 0,
    }))
}
