use animate_body::{HumanPose, MorphWeights};
use animate_hold::Holds;
use animate_track::Tracks;
use rikka_mmd::{VmdBoneFrame, VmdPropertyFrame};

use crate::morphs::Morphs;
use crate::motion::HumanMotion;
use crate::part::Part;
use crate::sample::PoseSample;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Take {
    pub human: Option<HumanMotion>,
    pub bones: Tracks<VmdBoneFrame>,
    pub switches: Tracks<VmdPropertyFrame>,
    pub morphs: Morphs,
    pub holds: Holds,
}

impl Take {
    #[must_use]
    pub fn last_frame(&self) -> u32 {
        let human = self.human.as_ref().map_or(0, HumanMotion::last_frame);
        [
            self.bones.last_frame(),
            self.switches.last_frame(),
            self.morphs.last_frame(),
        ]
        .into_iter()
        .flatten()
        .fold(human.max(self.holds.last_frame()), u32::max)
    }

    #[must_use]
    pub fn is_still(&self) -> bool {
        self.human.as_ref().is_none_or(HumanMotion::is_empty)
            && self.bones.is_empty()
            && self.switches.is_empty()
            && self.morphs.is_empty()
            && self.holds.is_empty()
    }

    #[must_use]
    pub fn sample(&self, frame: f32, rest: &HumanPose) -> PoseSample {
        match &self.human {
            Some(human) => human.sample(frame, rest),
            None => PoseSample::resting(rest),
        }
    }

    #[must_use]
    pub fn morph_weight(&self, morph: &str, frame: f32) -> f32 {
        self.morphs.value_at(morph, frame).unwrap_or(0.0)
    }

    #[must_use]
    pub fn morph_weights(&self, frame: f32) -> MorphWeights {
        self.morphs
            .keyed()
            .map(|morph| (morph.clone(), self.morph_weight(morph, frame)))
            .collect()
    }

    pub fn merge(&mut self, other: Take) {
        self.bones.merge(other.bones);
        self.switches.merge(other.switches);
        self.morphs.merge(other.morphs);
        for hold in other.holds.iter() {
            self.holds.lay(hold.clone());
        }
        let Some(incoming) = other.human else {
            return;
        };
        let Some(human) = &mut self.human else {
            self.human = Some(incoming);
            return;
        };
        for part in Part::ALL {
            for key in incoming.parts().keys(&part) {
                human.insert(key.clone());
            }
        }
    }
}
