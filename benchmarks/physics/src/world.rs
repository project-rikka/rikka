use std::time::Duration;

use crate::blueprint::{BodyDef, BodyIndex, JointDef, Pose};
use crate::error::Result;

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub workers: usize,
    pub sleeping: bool,
    pub sleep_threshold: f32,
    pub capacity: usize,
}

pub trait World {
    fn add_body(&mut self, body: &BodyDef) -> Result<BodyIndex>;

    fn add_joint(&mut self, joint: &JointDef) -> Result<()>;

    fn prepare(&mut self);

    fn move_kinematic(&mut self, body: BodyIndex, target: Pose);

    fn step(&mut self);

    fn pose(&self, body: BodyIndex) -> Pose;

    fn awake_bodies(&self) -> usize;

    fn state_hash(&self) -> Option<u64> {
        None
    }

    fn enable_profile(&mut self) {}

    fn profile(&self) -> Vec<(&'static str, Duration, u32)> {
        Vec::new()
    }
}

pub trait Engine: 'static {
    const NAME: &'static str;

    fn world(settings: &Settings) -> Result<Box<dyn World>>;
}
