use animate_inspector::MorphEditing;
use std::sync::Arc;

use animate_body::{Body, HandPreset, Limb, Side};
use animate_cast::{FigureId, Grip, Subject, Tuning, grip_muscles, grip_of};
use animate_edit::{Change, Key, KeyKind};
use animate_inspector::PoseEditing;
use animate_motion::{Automation, Goal, Part, PartKey, PartValues, Pinned, PoseSample, Tag};
use animate_sheet::Lane;
use animate_stage::{Pending, Posture};

use crate::animate::Animate;

const NOTICED: f32 = 1e-4;

impl Animate {
    pub(crate) fn chosen_part(&self) -> Option<Part> {
        match self.timeline.chosen_lane() {
            Some(Lane::Part(part)) => Some(part),
            _ => None,
        }
    }

    fn chosen_human(&self) -> Option<(FigureId, Arc<Body>)> {
        let Some(Subject::Figure(id)) = self.subject else {
            return None;
        };
        let body = self.cast.figure(id)?.body.clone()?;
        Some((id, body))
    }

    fn registered(&self, id: FigureId, body: &Body, frame: u32) -> PoseSample {
        self.take_of(id).sample(frame as f32, body.rest_pose())
    }

    pub(crate) fn preview(&mut self, pose_sample: PoseSample) {
        let Some((id, body)) = self.chosen_human() else {
            return;
        };
        let frame = self.transport.frame();
        if self.transport.is_playing() {
            self.transport.pause();
        }
        let still = pose_sample.still(&body, || self.eye().at(frame));
        self.stage.lock().preview(
            id,
            frame,
            Posture {
                pose_sample,
                shown: still,
            },
        );
    }

    pub(super) fn register_parts(&mut self) {
        let edited = self.unregistered_parts();
        self.register_edited(edited);
    }

    pub(super) fn register_edited(&mut self, edited: Vec<Part>) {
        let Some((id, body)) = self.chosen_human() else {
            return;
        };
        let Some(mut pose_sample) = self.sampled_now() else {
            return;
        };
        regoal(&body, &mut pose_sample, |limb, pinned| {
            !pinned || edited.contains(&Part::from(limb))
        });
        let frame = self.transport.frame();
        let morphs = self.morph_keys(id, frame, edited.is_empty());
        let parts = match (edited.is_empty(), morphs.is_empty(), self.chosen_part()) {
            (false, _, _) => edited,
            (true, false, _) => Vec::new(),
            (true, true, Some(part)) => vec![part],
            (true, true, None) => Part::ALL.to_vec(),
        };
        let take = self.take_of(id);
        let mut keys: Vec<Key> = parts
            .into_iter()
            .map(|part| {
                let standing = take
                    .human
                    .as_ref()
                    .and_then(|human| human.parts().key(&part, frame));
                Key::Part(PartKey {
                    part,
                    frame,
                    tag: standing.map_or(self.controls.tag, |standing| standing.tag),
                    values: pose_sample.values(part),
                    slopes: None,
                })
            })
            .collect();
        keys.extend(morphs);
        self.stage.lock().release();
        self.lay(Subject::Figure(id), keys);
    }

    pub(super) fn reset_parts(&mut self) {
        if let Some(morph) = self.chosen_morph() {
            self.zero_morphs(Some(&morph));
            return;
        }
        let (Some(body), Some(mut pose_sample)) = (self.chosen_body().cloned(), self.sampled_now())
        else {
            return;
        };
        let resting = PoseSample::resting(body.rest_pose());
        let chosen = self.chosen_part();
        let parts = match chosen {
            Some(part) => vec![part],
            None => Part::ALL.to_vec(),
        };
        for part in parts {
            pose_sample.set(part, &resting.values(part));
        }
        self.preview(pose_sample);
        if chosen.is_none() {
            self.zero_morphs(None);
        }
    }

    pub(crate) fn toggle_pin(&mut self, limb: Limb) {
        let Some(mut pose_sample) = self.sampled_now() else {
            return;
        };
        let Some(body) = self.chosen_body() else {
            return;
        };
        let pinned = &mut pose_sample.pins[limb.index()];
        *pinned = match pinned.pin {
            true => Pinned {
                pin: false,
                ..*pinned
            },
            false => Pinned::new(
                true,
                Goal::from_worlds(&body.fk(&pose_sample.pose), limb, body.scale()),
            ),
        };
        self.preview(pose_sample);
    }

    pub(crate) fn can_pin(&self) -> bool {
        self.is_easy_mode() && self.chosen_part().and_then(Part::limb).is_some()
    }

    pub(crate) fn pin_chosen(&mut self) {
        if let Some(limb) = self.chosen_part().and_then(Part::limb) {
            self.toggle_pin(limb);
        }
    }
}

fn regoal(body: &Body, pose_sample: &mut PoseSample, chosen: impl Fn(Limb, bool) -> bool) {
    let worlds = body.fk(&pose_sample.pose);
    for limb in Limb::ALL {
        let pinned = &mut pose_sample.pins[limb.index()];
        if chosen(limb, pinned.pin) {
            pinned.goal = Goal::from_worlds(&worlds, limb, body.scale());
        }
    }
}

pub(crate) fn differs(one: &PartValues, two: &PartValues) -> bool {
    let apart = |one: &[f32], two: &[f32]| {
        one.len() != two.len()
            || one
                .iter()
                .zip(two)
                .any(|(one, two)| (one - two).abs() > NOTICED)
    };
    match (one, two) {
        (PartValues::Muscles(one), PartValues::Muscles(two)) => apart(one, two),
        (
            PartValues::Limb {
                muscles: one,
                pin: one_pin,
                ..
            },
            PartValues::Limb {
                muscles: two,
                pin: two_pin,
                ..
            },
        ) => one_pin != two_pin || apart(one, two),
        (
            PartValues::Root {
                position: one_position,
                rotation: one_rotation,
            },
            PartValues::Root {
                position: two_position,
                rotation: two_rotation,
            },
        ) => {
            one_position.distance(*two_position) > NOTICED
                || one_rotation.angle_between(*two_rotation) > NOTICED
        }
        (
            PartValues::Gaze {
                eyes: one,
                gaze: one_gaze,
            },
            PartValues::Gaze {
                eyes: two,
                gaze: two_gaze,
            },
        ) => one_gaze != two_gaze || apart(one, two),
        (one, two) => one != two,
    }
}

impl PoseEditing for Animate {
    fn apply_tuning(&mut self, tuning: Tuning, value: f32) {
        self.controls.automation_draft = None;
        self.automate(|automation| tuning.set(automation, value));
    }
    fn automate(&mut self, change: &mut dyn FnMut(&mut Automation)) {
        Animate::automate(self, change);
    }
    fn automation(&self) -> Option<Automation> {
        let (id, _) = self.chosen_human()?;
        Some(
            self.take_of(id)
                .human
                .as_ref()
                .map_or_else(Automation::default, |human| human.settings.clone()),
        )
    }
    fn hand_preset(&mut self, side: Side, preset: HandPreset) {
        self.touch_with(|pose_sample| preset.apply(side, &mut pose_sample.pose));
    }
    fn limits(&self) -> bool {
        self.automation().is_none_or(|automation| automation.limits)
    }
    fn regrip(&mut self, side: Side, grip: Grip, value: f32) {
        let Some(mut pose_sample) = self.sampled_now() else {
            return;
        };
        let by = value - grip_of(&pose_sample, side, grip);
        for at in grip_muscles(side, grip) {
            pose_sample.pose.muscles[at] += by;
        }
        if let Some(body) = self.chosen_body() {
            body.clamp(&mut pose_sample.pose, self.limits());
        }
        self.preview(pose_sample);
    }
    fn reset_style(&mut self) {
        self.controls.automation_draft = None;
        self.automate(|automation| {
            let neutral = Automation::default();
            automation.fitting = neutral.fitting;
            automation.jiggle = neutral.jiggle;
            automation.intensity = neutral.intensity;
        });
    }
    fn retag(&mut self, tag: Tag) {
        if let Some((subject, keys)) = self.picked_keys()
            && keys.iter().any(|key| key.kind() == KeyKind::Part)
        {
            self.commit(subject, Change::tagging(keys, tag));
            return;
        }
        self.controls.tag = tag;
    }
    fn sampled_now(&self) -> Option<PoseSample> {
        let (id, body) = self.chosen_human()?;
        let frame = self.transport.frame();
        let pending = self
            .stage
            .lock()
            .pending_at(id, frame)
            .and_then(Pending::posture)
            .map(|posture| posture.pose_sample.clone());
        Some(pending.unwrap_or_else(|| self.registered(id, &body, frame)))
    }
    fn shown_tag(&self) -> Tag {
        self.picked_keys()
            .and_then(|(_, keys)| {
                keys.iter().find_map(|key| match key {
                    Key::Part(key) => Some(key.tag),
                    _ => None,
                })
            })
            .unwrap_or(self.controls.tag)
    }
    fn touch_with(&mut self, change: &mut dyn FnMut(&mut PoseSample)) {
        Animate::touch_with(self, change);
    }
    fn tuned(&self, tuning: Tuning) -> f32 {
        match self.controls.automation_draft {
            Some((drafted_tuning, value)) if drafted_tuning == tuning => value,
            _ => tuning.value(&self.automation().unwrap_or_default()),
        }
    }
    fn unregistered_parts(&self) -> Vec<Part> {
        let Some((id, body)) = self.chosen_human() else {
            return Vec::new();
        };
        let frame = self.transport.frame();
        let Some(edited) = self
            .stage
            .lock()
            .pending_at(id, frame)
            .and_then(Pending::posture)
            .map(|posture| posture.pose_sample.clone())
        else {
            return Vec::new();
        };
        let registered = self.registered(id, &body, frame);
        Part::ALL
            .into_iter()
            .filter(|part| differs(&registered.values(*part), &edited.values(*part)))
            .collect()
    }
}

impl Animate {
    pub(crate) fn automate(&mut self, change: impl FnOnce(&mut Automation)) {
        let Some((id, _)) = self.chosen_human() else {
            return;
        };
        let mut take = self.take_of(id);
        let human = Arc::make_mut(&mut take).human.get_or_insert_default();
        let before = human.settings.clone();
        change(&mut human.settings);
        if human.settings == before {
            return;
        }
        self.set_take(id, take);
        self.refresh();
    }

    pub(crate) fn touch_with(&mut self, change: impl FnOnce(&mut PoseSample)) {
        let Some(mut pose_sample) = self.sampled_now() else {
            return;
        };
        change(&mut pose_sample);
        self.preview(pose_sample);
    }
}
