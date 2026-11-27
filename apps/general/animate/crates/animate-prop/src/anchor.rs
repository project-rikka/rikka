use animate_track::Keyframe;
use rikka_keyframe::Framed;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Anchor {
    pub figure: String,
    pub bone: String,
}

impl Anchor {
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.figure.trim().is_empty() && !self.bone.trim().is_empty()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnchorKey {
    pub frame: u32,
    pub anchor: Option<Anchor>,
}

impl Framed for AnchorKey {
    type Frame = u32;

    fn frame(&self) -> u32 {
        self.frame
    }
}

impl Keyframe for AnchorKey {
    type Id = ();

    fn id(&self) -> &() {
        &()
    }

    fn at_frame(self, frame: u32) -> Self {
        Self { frame, ..self }
    }
}
