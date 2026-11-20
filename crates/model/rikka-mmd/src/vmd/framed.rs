use rikka_keyframe::Framed;

use super::document::{VmdBoneFrame, VmdCameraFrame, VmdPropertyFrame};

impl Framed for VmdBoneFrame {
    type Frame = u32;

    fn frame(&self) -> u32 {
        self.frame
    }
}

impl Framed for VmdCameraFrame {
    type Frame = u32;

    fn frame(&self) -> u32 {
        self.frame
    }
}

impl Framed for VmdPropertyFrame {
    type Frame = u32;

    fn frame(&self) -> u32 {
        self.frame
    }
}
