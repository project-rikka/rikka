use rikka_physics_geometry::Tolerance;
use rikka_physics_math::Vec3;

use crate::Spring;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Config {
    pub gravity: Vec3,
    pub substeps: u32,
    pub tolerance: Tolerance,
    pub contact_softness: Spring,
    pub contact_push_speed: f32,
    pub joint_softness: Spring,
    pub restitution_threshold: f32,
    pub hit_event_threshold: f32,
    pub max_linear_speed: f32,
    pub sleeping: bool,
    pub time_to_sleep: f32,
    pub warm_starting: bool,
    pub recycling: bool,
    pub continuous: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            gravity: Vec3::new(0.0, -9.81, 0.0),
            substeps: 4,
            tolerance: Tolerance::default(),
            contact_softness: Spring::hertz(30.0).damping_ratio(10.0),
            contact_push_speed: 3.0,
            joint_softness: Spring::hertz(60.0).damping_ratio(2.0),
            restitution_threshold: 1.0,
            hit_event_threshold: 1.0,
            max_linear_speed: 400.0,
            sleeping: true,
            time_to_sleep: 0.5,
            warm_starting: true,
            recycling: true,
            continuous: true,
        }
    }
}

impl Config {
    pub(crate) fn linear_slop(&self) -> f32 {
        self.tolerance.linear_slop()
    }

    pub(crate) fn speculative_cap(&self) -> f32 {
        self.tolerance.speculative_distance()
    }

    pub(crate) fn sleep_threshold(&self) -> f32 {
        0.05 * self.tolerance.length_scale
    }
}
