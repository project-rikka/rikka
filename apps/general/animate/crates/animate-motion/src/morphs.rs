use animate_track::{Keyframe, Sampled, Tracks};
use rikka_keyframe::Framed;
use rikka_mmd::VmdCurve;

#[derive(Clone, Debug, PartialEq)]
pub struct MorphKey {
    pub morph: String,
    pub frame: u32,
    pub weight: f32,
    pub curve: VmdCurve,
}

impl Framed for MorphKey {
    type Frame = u32;

    fn frame(&self) -> u32 {
        self.frame
    }
}

impl Keyframe for MorphKey {
    type Id = String;

    fn id(&self) -> &String {
        &self.morph
    }

    fn at_frame(self, frame: u32) -> Self {
        Self { frame, ..self }
    }
}

impl Sampled for MorphKey {
    type Value = f32;

    fn value(&self) -> f32 {
        self.weight
    }

    fn toward(&self, to: &Self, t: f32) -> f32 {
        self.weight + (to.weight - self.weight) * to.curve.factor(t)
    }
}

pub type Morphs = Tracks<MorphKey>;
