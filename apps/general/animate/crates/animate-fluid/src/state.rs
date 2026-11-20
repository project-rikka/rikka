use rikka_model_core::glam::{Mat4, Vec3};

use crate::Throw;
use crate::aim::Aim;
use crate::anchor::Spot;
use crate::levels::EmitterLevels;
use crate::setting::EmitterSetting;
use crate::shape::Shape;
use crate::value::EmitterValue;

#[derive(Clone, Debug, PartialEq)]
pub struct EmitterState {
    pub on: bool,
    pub shape: Shape,
    pub aim: Aim,
    pub launch: Spot,
    pub target: Spot,
    pub levels: EmitterLevels,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flight {
    pub from: Vec3,
    pub to: Vec3,
    pub throw: Throw,
}

impl Flight {
    #[must_use]
    pub fn path(&self, gravity: Vec3, points: usize) -> Vec<Vec3> {
        self.throw.path(self.from, gravity, points)
    }
}

impl EmitterState {
    #[must_use]
    pub fn spot(&self, setting: EmitterSetting) -> Option<&Spot> {
        match setting {
            EmitterSetting::Launch => Some(&self.launch),
            EmitterSetting::Target => Some(&self.target),
            _ => None,
        }
    }

    #[must_use]
    pub fn throw(&self, from: Vec3, to: Vec3, gravity: Vec3) -> Throw {
        if !self.shape.aims() {
            return Throw::STILL;
        }
        match self.aim {
            Aim::Ballistic => Throw::ballistic(from, to, gravity, self.levels.arc)
                .unwrap_or_else(|| Throw::direct(from, to, gravity, self.levels.speed)),
            Aim::Direction => Throw::direct(from, to, gravity, self.levels.speed),
        }
    }

    #[must_use]
    pub fn flight(
        &self,
        gravity: Vec3,
        locate: impl Fn(&str, &str) -> Option<Mat4>,
    ) -> Option<Flight> {
        if !self.shape.launches() {
            return None;
        }
        let from = self.launch.world(&locate)?;
        let to = self
            .shape
            .aims()
            .then(|| self.target.world(&locate))
            .flatten();
        self.flight_between(from, to, gravity)
    }

    #[must_use]
    pub fn flight_between(&self, from: Vec3, to: Option<Vec3>, gravity: Vec3) -> Option<Flight> {
        let to = match self.shape.aims() {
            true => to?,
            false => from,
        };
        Some(Flight {
            from,
            to,
            throw: self.throw(from, to, gravity),
        })
    }

    pub fn set(&mut self, setting: EmitterSetting, value: EmitterValue) {
        match (setting, value) {
            (EmitterSetting::Launch, EmitterValue::Three(three)) => {
                self.launch.offset = Vec3::from_array(three)
            }
            (EmitterSetting::Target, EmitterValue::Three(three)) => {
                self.target.offset = Vec3::from_array(three)
            }
            (_, EmitterValue::Number(level)) => self.levels.set(setting, level),
            _ => {}
        }
    }
}
