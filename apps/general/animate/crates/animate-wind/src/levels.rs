use rikka_model_core::glam::{Quat, Vec3};
use rikka_physics_tailor::Wind;

use crate::setting::WindSetting;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindLevels {
    pub yaw: f32,
    pub pitch: f32,
    pub speed: f32,
    pub turbulence: f32,
    pub gust: f32,
}

impl Default for WindLevels {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            speed: 0.0,
            turbulence: 0.3,
            gust: 0.5,
        }
    }
}

impl WindLevels {
    #[must_use]
    pub fn get(&self, setting: WindSetting) -> f32 {
        match setting {
            WindSetting::Yaw => self.yaw,
            WindSetting::Pitch => self.pitch,
            WindSetting::Speed => self.speed,
            WindSetting::Turbulence => self.turbulence,
            WindSetting::Gust => self.gust,
        }
    }

    pub fn set(&mut self, setting: WindSetting, value: f32) {
        let slot = match setting {
            WindSetting::Yaw => &mut self.yaw,
            WindSetting::Pitch => &mut self.pitch,
            WindSetting::Speed => &mut self.speed,
            WindSetting::Turbulence => &mut self.turbulence,
            WindSetting::Gust => &mut self.gust,
        };
        *slot = value;
    }

    #[must_use]
    pub fn direction(&self) -> Vec3 {
        let turn = Quat::from_rotation_y(self.yaw.to_radians())
            * Quat::from_rotation_x(self.pitch.to_radians());
        turn * Vec3::NEG_Z
    }

    #[must_use]
    pub fn wind(&self) -> Wind {
        Wind {
            direction: self.direction(),
            speed: self.speed,
            turbulence: self.turbulence,
            gust: self.gust,
        }
    }
}
