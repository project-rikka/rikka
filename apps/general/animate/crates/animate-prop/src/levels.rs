use rikka_model_core::glam::{EulerRot, Mat4, Quat, Vec3};

use crate::setting::PropSetting;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PropLevels {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub roll: f32,
    pub scale: f32,
    pub visible: bool,
}

impl Default for PropLevels {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            pitch: 0.0,
            yaw: 0.0,
            roll: 0.0,
            scale: 1.0,
            visible: true,
        }
    }
}

impl PropLevels {
    #[must_use]
    pub fn get(&self, setting: PropSetting) -> f32 {
        match setting {
            PropSetting::X => self.x,
            PropSetting::Y => self.y,
            PropSetting::Z => self.z,
            PropSetting::Pitch => self.pitch,
            PropSetting::Yaw => self.yaw,
            PropSetting::Roll => self.roll,
            PropSetting::Scale => self.scale,
            PropSetting::Visible => f32::from(u8::from(self.visible)),
        }
    }

    pub fn set(&mut self, setting: PropSetting, value: f32) {
        let slot = match setting {
            PropSetting::X => &mut self.x,
            PropSetting::Y => &mut self.y,
            PropSetting::Z => &mut self.z,
            PropSetting::Pitch => &mut self.pitch,
            PropSetting::Yaw => &mut self.yaw,
            PropSetting::Roll => &mut self.roll,
            PropSetting::Scale => &mut self.scale,
            PropSetting::Visible => {
                self.visible = value >= 0.5;
                return;
            }
        };
        *slot = value;
    }

    #[must_use]
    pub fn position(&self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }

    #[must_use]
    pub fn rotation(&self) -> Quat {
        Quat::from_euler(
            EulerRot::YXZ,
            self.yaw.to_radians(),
            self.pitch.to_radians(),
            self.roll.to_radians(),
        )
    }

    #[must_use]
    pub fn transform(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            Vec3::splat(self.scale),
            self.rotation(),
            self.position(),
        )
    }
}
