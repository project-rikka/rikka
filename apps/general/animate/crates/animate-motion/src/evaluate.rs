use animate_body::{Body, HumanPose, Limb};
use animate_solve::{ground, look, reach};
use rikka_model_core::glam::Vec3;

use crate::motion::HumanMotion;
use crate::part::GazeMode;
use crate::sample::PoseSample;
use crate::secondary::Springs;

#[derive(Debug)]
pub struct Evaluator<'a> {
    body: &'a Body,
    motion: &'a HumanMotion,
    springs: Springs,
}

impl<'a> Evaluator<'a> {
    #[must_use]
    pub fn new(body: &'a Body, motion: &'a HumanMotion) -> Self {
        Self::resume(body, motion, Springs::default())
    }

    pub(crate) fn resume(body: &'a Body, motion: &'a HumanMotion, springs: Springs) -> Self {
        Self {
            body,
            motion,
            springs,
        }
    }

    pub(crate) fn springs(&self) -> &Springs {
        &self.springs
    }

    pub fn evaluate(&mut self, frame: u32, eye: Vec3) -> HumanPose {
        let pose_sample = self.motion.sample(frame as f32, self.body.rest_pose());
        self.evaluate_sampled(pose_sample, eye)
    }

    pub(crate) fn evaluate_sampled(&mut self, pose_sample: PoseSample, eye: Vec3) -> HumanPose {
        let lagged = self.lag(&pose_sample);
        settle(self.body, lagged, &pose_sample, || eye)
    }

    pub(crate) fn lag(&mut self, pose_sample: &PoseSample) -> HumanPose {
        let mut pose = pose_sample.pose;
        self.springs.lag(
            self.body,
            &mut pose,
            &pose_sample.pins,
            self.motion.settings.secondary,
        );
        pose
    }
}

impl PoseSample {
    #[must_use]
    pub fn still(&self, body: &Body, eye: impl FnOnce() -> Vec3) -> HumanPose {
        let mut pose = self.pose;
        stand(body, &mut pose, self, eye);
        pose
    }

    #[must_use]
    pub fn reached(&self, body: &Body, eye: impl FnOnce() -> Vec3) -> HumanPose {
        settle(body, self.pose, self, eye)
    }
}

pub(crate) fn settle(
    body: &Body,
    mut pose: HumanPose,
    pose_sample: &PoseSample,
    eye: impl FnOnce() -> Vec3,
) -> HumanPose {
    for limb in Limb::ALL {
        let pinned = pose_sample.pins[limb.index()];
        if pinned.weight > 0.0 {
            reach(
                body,
                &mut pose,
                limb,
                pinned.goal.position * body.scale(),
                Some(pinned.goal.rotation),
                pinned.weight,
            );
        }
    }
    stand(body, &mut pose, pose_sample, eye);
    pose
}

fn stand(body: &Body, pose: &mut HumanPose, pose_sample: &PoseSample, eye: impl FnOnce() -> Vec3) {
    ground(body, pose);
    let gaze = pose_sample.gaze;
    let target = match gaze.mode {
        GazeMode::Off => None,
        GazeMode::Point => Some(gaze.target * body.scale()),
        GazeMode::Camera => Some(eye()),
    };
    if let Some(target) = target {
        look(body, pose, target, gaze.weight, gaze.head_follow);
    }
}
