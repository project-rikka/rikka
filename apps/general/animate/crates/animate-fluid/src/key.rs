use animate_track::{Keyframe, Sampled};
use rikka_keyframe::Framed;
use rikka_mmd::VmdCurve;

use crate::setting::EmitterSetting;
use crate::value::EmitterValue;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmitterKey {
    pub setting: EmitterSetting,
    pub frame: u32,
    pub value: EmitterValue,
    pub curve: VmdCurve,
}

impl Framed for EmitterKey {
    type Frame = u32;

    fn frame(&self) -> u32 {
        self.frame
    }
}

impl Keyframe for EmitterKey {
    type Id = EmitterSetting;

    fn id(&self) -> &EmitterSetting {
        &self.setting
    }

    fn at_frame(self, frame: u32) -> Self {
        Self { frame, ..self }
    }
}

impl Sampled for EmitterKey {
    type Value = EmitterValue;

    fn value(&self) -> EmitterValue {
        self.value
    }

    fn toward(&self, to: &Self, t: f32) -> EmitterValue {
        self.value.mix(to.value, to.curve.factor(t))
    }
}
