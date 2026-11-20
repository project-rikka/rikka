use glam::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wind {
    pub direction: Vec3,
    pub speed: f32,
    pub turbulence: f32,
    pub gust: f32,
}

impl Wind {
    pub const CALM: Wind = Wind {
        direction: Vec3::NEG_Z,
        speed: 0.0,
        turbulence: 0.0,
        gust: 0.0,
    };

    #[must_use]
    pub fn velocity(&self) -> Vec3 {
        self.direction.normalize_or_zero() * self.speed
    }
}

impl Default for Wind {
    fn default() -> Wind {
        Wind::CALM
    }
}
