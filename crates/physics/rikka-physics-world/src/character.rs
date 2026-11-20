use std::f32::consts::FRAC_PI_4;

use rikka_physics_collision::{Mover, QueryFilter};
use rikka_physics_math::Vec3;
use rikka_physics_rigid::Character;

use crate::{Error, Result};

rikka_physics_core::id!(pub CharacterId);

#[must_use]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharacterDef {
    pub(crate) mover: Mover,
    pub(crate) position: Vec3,
    pub(crate) up: Vec3,
    pub(crate) max_slope: f32,
    pub(crate) step_height: f32,
    pub(crate) filter: QueryFilter,
}

impl CharacterDef {
    pub fn new(mover: Mover, position: Vec3) -> Self {
        Self {
            mover,
            position,
            up: Vec3::Y,
            max_slope: FRAC_PI_4,
            step_height: 0.0,
            filter: QueryFilter::default(),
        }
    }

    pub fn capsule(radius: f32, height: f32, position: Vec3) -> Self {
        let half = (0.5 * height - radius).max(0.0);
        Self::new(
            Mover::new(
                Vec3::new(0.0, radius, 0.0),
                Vec3::new(0.0, radius + 2.0 * half, 0.0),
                radius,
            ),
            position,
        )
    }

    pub fn up(mut self, up: Vec3) -> Self {
        self.up = up;
        self
    }

    pub fn max_slope(mut self, radians: f32) -> Self {
        self.max_slope = radians;
        self
    }

    pub fn step_height(mut self, height: f32) -> Self {
        self.step_height = height;
        self
    }

    pub fn filter(mut self, filter: QueryFilter) -> Self {
        self.filter = filter;
        self
    }

    pub(crate) fn build(&self) -> Result<Character> {
        let finite = self.mover.center1.is_finite()
            && self.mover.center2.is_finite()
            && self.position.is_finite()
            && self.up.is_finite()
            && self.max_slope.is_finite()
            && self.step_height.is_finite();
        if !finite {
            return Err(Error::NotFinite {
                what: "character definition",
            });
        }
        if !(self.mover.radius.is_finite() && self.mover.radius > 0.0) {
            return Err(Error::NotPositive {
                what: "character radius",
                value: self.mover.radius as f64,
            });
        }
        Ok(Character::new(self.mover, self.position)
            .up(self.up)
            .max_slope(self.max_slope)
            .step_height(self.step_height)
            .filter(self.filter))
    }
}
