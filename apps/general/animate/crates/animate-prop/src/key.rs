use animate_track::{Keyframe, Sampled};
use rikka_keyframe::Framed;
use rikka_mmd::VmdCurve;

use crate::setting::PropSetting;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PropKey {
    pub setting: PropSetting,
    pub frame: u32,
    pub value: f32,
    pub curve: VmdCurve,
}

impl Framed for PropKey {
    type Frame = u32;

    fn frame(&self) -> u32 {
        self.frame
    }
}

impl Keyframe for PropKey {
    type Id = PropSetting;

    fn id(&self) -> &PropSetting {
        &self.setting
    }

    fn at_frame(self, frame: u32) -> Self {
        Self { frame, ..self }
    }
}

impl Sampled for PropKey {
    type Value = f32;

    fn value(&self) -> f32 {
        self.value
    }

    fn toward(&self, to: &Self, t: f32) -> f32 {
        self.setting.mix(self.value, to.value, to.curve.factor(t))
    }
}
