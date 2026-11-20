use std::fmt::Debug;

use rikka_keyframe::Framed;
use rikka_mmd::{VmdBoneFrame, VmdCameraFrame, VmdCameraPose, VmdPropertyFrame};
use rikka_model_core::glam::{Quat, Vec3};

pub trait Keyframe: Framed<Frame = u32> + Clone + PartialEq {
    type Id: Clone + Ord + Debug;

    fn id(&self) -> &Self::Id;

    #[must_use]
    fn at_frame(self, frame: u32) -> Self;
}

pub trait Sampled: Keyframe {
    type Value;

    fn value(&self) -> Self::Value;

    fn toward(&self, to: &Self, t: f32) -> Self::Value;
}

impl Keyframe for VmdBoneFrame {
    type Id = String;

    fn id(&self) -> &String {
        &self.bone
    }

    fn at_frame(self, frame: u32) -> Self {
        Self { frame, ..self }
    }
}

impl Sampled for VmdBoneFrame {
    type Value = (Vec3, Quat);

    fn value(&self) -> (Vec3, Quat) {
        (self.translation, self.rotation)
    }

    fn toward(&self, to: &Self, t: f32) -> (Vec3, Quat) {
        VmdBoneFrame::toward(self, to, t)
    }
}

impl Keyframe for VmdCameraFrame {
    type Id = ();

    fn id(&self) -> &() {
        &()
    }

    fn at_frame(self, frame: u32) -> Self {
        Self { frame, ..self }
    }
}

impl Sampled for VmdCameraFrame {
    type Value = VmdCameraPose;

    fn value(&self) -> VmdCameraPose {
        self.pose()
    }

    fn toward(&self, to: &Self, t: f32) -> VmdCameraPose {
        VmdCameraFrame::toward(self, to, t)
    }
}

impl Keyframe for VmdPropertyFrame {
    type Id = ();

    fn id(&self) -> &() {
        &()
    }

    fn at_frame(self, frame: u32) -> Self {
        Self { frame, ..self }
    }
}
