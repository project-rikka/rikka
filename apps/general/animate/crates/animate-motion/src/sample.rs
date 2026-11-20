use animate_body::{EYES, HumanPose};

use crate::curve::Curve;
use crate::motion::HumanMotion;
use crate::part::{Gaze, Goal, Part, PartValues};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pinned {
    pub pin: bool,
    pub weight: f32,
    pub goal: Goal,
}

impl Pinned {
    #[must_use]
    pub fn new(pin: bool, goal: Goal) -> Self {
        Self {
            pin,
            weight: if pin { 1.0 } else { 0.0 },
            goal,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PoseSample {
    pub pose: HumanPose,
    pub pins: [Pinned; 4],
    pub gaze: Gaze,
}

impl PoseSample {
    #[must_use]
    pub fn resting(rest: &HumanPose) -> Self {
        Self {
            pose: *rest,
            pins: [Pinned::default(); 4],
            gaze: Gaze::default(),
        }
    }

    #[must_use]
    pub fn values(&self, part: Part) -> PartValues {
        if let Some(limb) = part.limb() {
            let pinned = self.pins[limb.index()];
            return PartValues::Limb {
                muscles: self.pose.muscles[limb.muscles()].to_vec(),
                pin: pinned.pin,
                goal: pinned.goal,
            };
        }
        if part == Part::Gaze {
            return PartValues::Gaze {
                eyes: self.pose.muscles[EYES].to_vec(),
                gaze: self.gaze,
            };
        }
        if let Some(range) = part.muscles() {
            return PartValues::Muscles(self.pose.muscles[range].to_vec());
        }
        PartValues::Root {
            position: self.pose.root_pos,
            rotation: self.pose.root_rot,
        }
    }

    pub fn set(&mut self, part: Part, values: &PartValues) {
        match values {
            PartValues::Root { position, rotation } => {
                self.pose.root_pos = *position;
                self.pose.root_rot = *rotation;
            }
            PartValues::Muscles(muscles) => self.set_muscles(part, muscles),
            PartValues::Limb { muscles, pin, goal } => {
                self.set_muscles(part, muscles);
                if let Some(limb) = part.limb() {
                    self.pins[limb.index()] = Pinned::new(*pin, *goal);
                }
            }
            PartValues::Gaze { eyes, gaze } => {
                self.set_muscles(part, eyes);
                self.gaze = *gaze;
            }
        }
    }

    fn set_muscles(&mut self, part: Part, muscles: &[f32]) {
        if let Some(range) = part.muscles() {
            for (slot, value) in self.pose.muscles[range].iter_mut().zip(muscles) {
                *slot = *value;
            }
        }
    }
}

impl HumanMotion {
    #[must_use]
    pub fn sample(&self, frame: f32, rest: &HumanPose) -> PoseSample {
        let mut pose_sample = PoseSample::resting(rest);
        for part in Part::ALL {
            let Some(curve) = Curve::locate(self.parts().keys(&part), frame) else {
                continue;
            };
            pose_sample.set(part, &sampled_values(&curve));
            if let Some(limb) = part.limb() {
                pose_sample.pins[limb.index()].weight = curve.blend(|key| key.values.pin());
            }
        }
        pose_sample
    }
}

pub(crate) fn sampled_values(curve: &Curve<'_>) -> PartValues {
    let mut values = curve.held_key().values.clone();
    for channel in 0..values.scalar_count() {
        let pose_sample = curve.scalar(
            |key| key.values.scalar(channel),
            |slopes| slopes.scalars.get(channel).copied(),
        );
        values.set_scalar(channel, pose_sample);
    }
    if values.turn().is_some() {
        let turn = curve.rotation(
            |key| key.values.turn().unwrap_or_default(),
            |slopes| slopes.turn,
        );
        values.set_turn(turn);
    }
    values
}
