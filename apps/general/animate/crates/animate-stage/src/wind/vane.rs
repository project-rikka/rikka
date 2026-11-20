use animate_wind::Wind;
use rikka_model::glam::{Mat4, Vec3};
use rikka_types::Point;

use crate::lens::Lens;

const SHORTEST: f32 = 0.3;
const LONGEST: f32 = 2.0;
const PER_METRE_PER_SECOND: f32 = 0.1;
const DEAD_ZONE: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vane {
    pub view_projection: Mat4,
    pub anchor: Vec3,
    pub wind: Wind,
}

impl Vane {
    pub fn length(&self) -> f32 {
        (SHORTEST + self.wind.speed * PER_METRE_PER_SECOND).min(LONGEST)
    }

    pub fn heading(&self) -> Vec3 {
        self.wind.direction.normalize_or(Vec3::NEG_Z)
    }

    pub fn tip(&self) -> Vec3 {
        self.anchor + self.heading() * self.length() * 0.5
    }

    pub fn tail(&self) -> Vec3 {
        self.anchor - self.heading() * self.length() * 0.5
    }

    pub fn yaw_at(&self, lens: &Lens, at: Point) -> Option<f32> {
        let centre = lens.project(self.anchor)?;
        let (across, down) = (at.x - centre.x, at.y - centre.y);
        if across.hypot(down) < DEAD_ZONE {
            return None;
        }
        let (right, up, _) = lens.sideways(self.anchor);
        let flat = |along: Vec3| Vec3::new(along.x, 0.0, along.z);
        let right = flat(right).normalize_or(Vec3::X);
        let away = (flat(up) + flat(lens.ahead(self.anchor))).normalize_or(Vec3::NEG_Z);
        let toward = right * across - away * down;
        Some((-toward.x).atan2(-toward.z).to_degrees().round())
    }
}
