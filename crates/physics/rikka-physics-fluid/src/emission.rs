use glam::Vec3;

use crate::body::RegionId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Nozzle {
    Stream { radius: f32 },
    Shower { radius: f32, spread: f32 },
    Drip { radius: f32 },
    Blob,
    Surface { region: RegionId, speed: f32 },
}

#[must_use]
#[derive(Clone, Debug, PartialEq)]
pub struct Emission {
    pub(crate) emitter: u32,
    pub(crate) nozzle: Nozzle,
    pub(crate) from: Vec3,
    pub(crate) to: Vec3,
    pub(crate) velocity: Vec3,
    pub(crate) amount: f32,
    pub(crate) substance: u32,
    pub(crate) lifetime: Option<f32>,
}

impl Emission {
    pub fn new(emitter: u32, nozzle: Nozzle) -> Emission {
        Emission {
            emitter,
            nozzle,
            from: Vec3::ZERO,
            to: Vec3::ZERO,
            velocity: Vec3::ZERO,
            amount: 0.0,
            substance: 0,
            lifetime: None,
        }
    }

    pub fn at(self, position: Vec3) -> Emission {
        self.moving(position, position)
    }

    pub fn moving(mut self, from: Vec3, to: Vec3) -> Emission {
        self.from = from;
        self.to = to;
        self
    }

    pub fn velocity(mut self, velocity: Vec3) -> Emission {
        self.velocity = velocity;
        self
    }

    pub fn rate(mut self, cubic_metres_per_second: f32) -> Emission {
        self.amount = cubic_metres_per_second;
        self
    }

    pub fn volume(mut self, cubic_metres: f32) -> Emission {
        self.amount = cubic_metres;
        self
    }

    pub fn substance(mut self, index: u32) -> Emission {
        self.substance = index;
        self
    }

    pub fn lifetime(mut self, seconds: f32) -> Emission {
        self.lifetime = Some(seconds);
        self
    }

    #[must_use]
    pub fn emitter(&self) -> u32 {
        self.emitter
    }

    #[must_use]
    pub fn nozzle(&self) -> Nozzle {
        self.nozzle
    }
}
