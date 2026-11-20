use std::time::Duration;

use crate::blueprint::{BodyIndex, Pose};
use crate::error::Result;
use crate::scene::{Driver, Scene, Setup};
use crate::size::Size;
use crate::world::{Engine, Settings, World};

pub struct Stage {
    world: Box<dyn World>,
    driver: Option<Box<dyn Driver>>,
    bodies: usize,
    steps: u32,
}

impl Stage {
    pub fn new<E: Engine>(scene: &Scene, size: Size, workers: usize) -> Result<Self> {
        Self::from_setup::<E>(scene.setup(size)?, workers)
    }

    pub fn from_setup<E: Engine>(setup: Setup, workers: usize) -> Result<Self> {
        let Setup { blueprint, driver } = setup;
        let mut world = E::world(&Settings {
            workers,
            sleeping: blueprint.sleeping,
            sleep_threshold: blueprint.sleep_threshold,
            capacity: blueprint.capacity(),
        })?;
        for (expected, body) in blueprint.bodies.iter().enumerate() {
            let index = world.add_body(body)?;
            debug_assert_eq!(index, BodyIndex(expected as u32));
        }
        for joint in &blueprint.joints {
            world.add_joint(joint)?;
        }
        world.prepare();
        Ok(Self {
            world,
            driver,
            bodies: blueprint.bodies.len(),
            steps: 0,
        })
    }

    pub fn step(&mut self) -> Result<()> {
        if let Some(driver) = self.driver.as_mut() {
            driver.drive(self.world.as_mut(), self.steps)?;
        }
        self.world.step();
        self.steps += 1;
        Ok(())
    }

    pub fn run(&mut self, steps: u32) -> Result<()> {
        (0..steps).try_for_each(|_| self.step())
    }

    pub fn bodies(&self) -> usize {
        self.bodies
    }

    pub fn pose(&self, body: BodyIndex) -> Pose {
        self.world.pose(body)
    }

    pub fn awake_bodies(&self) -> usize {
        self.world.awake_bodies()
    }

    pub fn state_hash(&self) -> Option<u64> {
        self.world.state_hash()
    }

    pub fn enable_profile(&mut self) {
        self.world.enable_profile();
    }

    pub fn profile(&self) -> Vec<(&'static str, Duration, u32)> {
        self.world.profile()
    }
}

impl std::fmt::Debug for Stage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Stage")
            .field("bodies", &self.bodies)
            .field("steps", &self.steps)
            .finish()
    }
}
