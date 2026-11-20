use rikka_model_core::glam::{Quat, Vec3};

use super::document::{FRAMES_PER_SECOND, Vmd, VmdCameraFrame};
use crate::euler;

const NARROWEST: u32 = 1;
const WIDEST: u32 = 179;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VmdCameraPose {
    pub target: Vec3,
    pub angles: Vec3,
    pub distance: f32,
    pub fov: f32,
    pub perspective: bool,
}

impl VmdCameraPose {
    #[must_use]
    pub fn rotation(&self) -> Quat {
        euler::rotation_of(-self.angles)
    }

    #[must_use]
    pub fn eye(&self) -> Vec3 {
        self.target + self.rotation() * Vec3::new(0.0, 0.0, -self.distance)
    }
}

impl VmdCameraFrame {
    #[must_use]
    pub fn pose(&self) -> VmdCameraPose {
        VmdCameraPose {
            target: self.target,
            angles: self.rotation,
            distance: self.distance,
            fov: view_angle(self.angle as f32),
            perspective: self.perspective,
        }
    }

    #[must_use]
    pub fn toward(&self, to: &VmdCameraFrame, factor: f32) -> VmdCameraPose {
        if to.frame.saturating_sub(self.frame) <= 1 {
            return self.pose();
        }
        let curves = to.interpolation;
        let mut target = Vec3::ZERO;
        for (axis, curve) in [curves.x, curves.y, curves.z].into_iter().enumerate() {
            target[axis] = mix(self.target[axis], to.target[axis], curve.factor(factor));
        }
        let turn = curves.rotation.factor(factor);
        VmdCameraPose {
            target,
            angles: self.rotation + (to.rotation - self.rotation) * turn,
            distance: mix(self.distance, to.distance, curves.distance.factor(factor)),
            fov: view_angle(mix(
                self.angle as f32,
                to.angle as f32,
                curves.angle.factor(factor),
            )),
            perspective: self.perspective,
        }
    }
}

#[derive(Clone, Debug)]
pub struct VmdCamera {
    frames: Vec<VmdCameraFrame>,
}

impl VmdCamera {
    #[must_use]
    pub fn frames(&self) -> &[VmdCameraFrame] {
        &self.frames
    }

    #[must_use]
    pub fn at(&self, time: f32) -> VmdCameraPose {
        let frame = time * FRAMES_PER_SECOND;
        let next = self
            .frames
            .partition_point(|keyframe| keyframe.frame as f32 <= frame);
        let Some(before) = next.checked_sub(1) else {
            return self.frames[0].pose();
        };
        let (from, Some(to)) = (&self.frames[before], self.frames.get(next)) else {
            return self.frames[before].pose();
        };
        let span = to.frame - from.frame;
        from.toward(to, (frame - from.frame as f32) / span as f32)
    }
}

impl Vmd {
    #[must_use]
    pub fn camera(&self) -> Option<VmdCamera> {
        if self.cameras.is_empty() {
            return None;
        }
        let mut frames = self.cameras.clone();
        frames.sort_by_key(|frame| frame.frame);
        frames.dedup_by_key(|frame| frame.frame);
        Some(VmdCamera { frames })
    }
}

fn mix(from: f32, to: f32, factor: f32) -> f32 {
    from + (to - from) * factor
}

fn view_angle(degrees: f32) -> f32 {
    degrees.clamp(NARROWEST as f32, WIDEST as f32).to_radians()
}
