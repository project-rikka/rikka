use animate_track::{Keyframe, Sampled};
use rikka_keyframe::Framed;
use rikka_mmd::VmdCurve;

use crate::setting::WindSetting;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindKey {
    pub setting: WindSetting,
    pub frame: u32,
    pub value: f32,
    pub curve: VmdCurve,
}

impl Framed for WindKey {
    type Frame = u32;

    fn frame(&self) -> u32 {
        self.frame
    }
}

impl Keyframe for WindKey {
    type Id = WindSetting;

    fn id(&self) -> &WindSetting {
        &self.setting
    }

    fn at_frame(self, frame: u32) -> Self {
        Self { frame, ..self }
    }
}

impl Sampled for WindKey {
    type Value = f32;

    fn value(&self) -> f32 {
        self.value
    }

    fn toward(&self, to: &Self, t: f32) -> f32 {
        self.setting.mix(self.value, to.value, to.curve.factor(t))
    }
}
