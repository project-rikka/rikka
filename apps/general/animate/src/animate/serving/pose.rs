use std::sync::Arc;

use animate_assistant::{Extremity, Hips, PoseKey};
use animate_body::{Body, Faces, Limb, Morph, MorphWeights, Side, muscle_of, muscles};
use animate_cast::Subject;
use animate_edit::{Change, Key};
use animate_motion::{GazeMode, Goal, Part, PartKey, Pinned, PoseSample, Tag};
use animate_solve::{DragScope, DragSession, Handle, Pin};
use rikka_model::glam::{EulerRot, Quat};
use rikka_tool::Reply;
use serde_json::json;

use super::{ServeError, ToolResult, round};
use crate::animate::Animate;
use crate::animate::posing::differs;
use crate::animate::subject_motion::SubjectMotion;

const REACHING: u32 = 12;

struct TakePlayback {
    parts: Vec<Part>,
    clamped: Vec<String>,
}

impl Animate {
    pub(super) fn pose_keys(&mut self, name: &str, keys: Vec<PoseKey>) -> ToolResult {
        let (id, body) = self.human(name)?;
        let faces = self.faces_of(id);
        let limits = self.limits_of(id);
        for key in &keys {
            if let Some((unknown, _)) = key
                .morphs
                .iter()
                .find(|(morph, _)| faces.morph(morph).is_none())
            {
                let morph_names: Vec<&str> = faces.morphs().iter().map(Morph::name).collect();
                return Err(ServeError::UnknownMorph {
                    frame: key.frame,
                    figure: name.to_owned(),
                    morph: unknown.clone(),
                    morphs: morph_names.join(", "),
                });
            }
            if let Some((missing, _)) = key
                .expressions
                .iter()
                .find(|(expression, _)| !faces.has(*expression))
            {
                return Err(ServeError::MissingExpression {
                    frame: key.frame,
                    figure: name.to_owned(),
                    expression: missing.name(),
                });
            }
        }
        let has_pending = self
            .stage
            .lock()
            .pending_at(id, self.transport.frame())
            .is_some();
        if has_pending {
            self.stage.lock().release();
        }
        let mut report = Vec::with_capacity(keys.len());
        for key in keys {
            let frame = key.frame;
            let mut pose_sample = self.sampled_at(id, &body, frame);
            let keyed_pose = pose(&body, &mut pose_sample, &key, limits);
            let take = self.take_of(id);
            let mut new_keys: Vec<Key> = keyed_pose
                .parts
                .iter()
                .map(|part| {
                    let standing = take
                        .human
                        .as_ref()
                        .and_then(|human| human.parts().key(part, frame));
                    Key::Part(PartKey {
                        part: *part,
                        frame,
                        tag: key
                            .tag
                            .or(standing.map(|standing| standing.tag))
                            .unwrap_or(Tag::Smooth),
                        values: pose_sample.values(*part),
                        slopes: None,
                    })
                })
                .collect();
            let expressed = expressed(&faces, &key);
            let expressed_morphs: Vec<String> = expressed
                .iter()
                .map(|(morph, _)| morph.to_owned())
                .collect();
            new_keys.extend(
                expressed
                    .iter()
                    .map(|(morph, weight)| self.morph_key(&take, morph, frame, weight)),
            );
            let subject_motion = SubjectMotion::Take(Arc::clone(&take));
            self.commit(
                Subject::Figure(id),
                Change::laying(&subject_motion, new_keys),
            );
            let sampled = self.sampled_at(id, &body, frame);
            let mut reported = json!({
                "frame": frame,
                "keyed": keyed_pose.parts.iter().map(|part| part.name()).collect::<Vec<&str>>(),
                "points": self.points(&body, &sampled, frame),
            });
            if !expressed_morphs.is_empty() {
                reported["morphs"] = json!(expressed_morphs);
            }
            if !keyed_pose.clamped.is_empty() {
                reported["clamped"] = json!(keyed_pose.clamped);
            }
            report.push(reported);
        }
        self.refresh();
        Ok(Reply::json(&json!({ "figure": name, "keys": report })))
    }
}

fn expressed(faces: &Faces, key: &PoseKey) -> MorphWeights {
    let mut weights = MorphWeights::default();
    for (expression, weight) in &key.expressions {
        faces.express(&mut weights, *expression, *weight);
    }
    for (morph, weight) in &key.morphs {
        weights.set(morph, *weight);
    }
    weights
}

fn pose(
    body: &Arc<Body>,
    pose_sample: &mut PoseSample,
    key: &PoseKey,
    limits: bool,
) -> TakePlayback {
    let before = pose_sample.clone();
    let scale = body.scale();
    let mut clamped = Vec::new();

    let resting = PoseSample::resting(body.rest_pose());
    for part in &key.rest {
        pose_sample.set(*part, &resting.values(*part));
    }

    for turn in &key.joints {
        let Some(index) = muscle_of(turn.bone, turn.dof) else {
            continue;
        };
        let muscle = muscles()[index];
        let value = muscle.value(turn.degrees.to_radians());
        let kept_value = muscle.keep(value, limits);
        if kept_value != value {
            clamped.push(format!(
                "{}.{} held at {}°",
                turn.bone.name(),
                animate_assistant::dof_name(turn.dof),
                round(muscle.angle(kept_value).to_degrees())
            ));
        }
        pose_sample.pose.muscles[index] = kept_value;
    }

    if let Some(hips) = key.hips {
        place_hips(pose_sample, hips, scale);
    }

    for (side, preset) in &key.hands {
        preset.apply(*side, &mut pose_sample.pose);
    }

    if let Some(gaze) = key.gaze {
        let mut turned = false;
        for side in Side::BOTH {
            if let Some(eye_direction) = gaze.eye(side) {
                eye_direction.apply(&mut pose_sample.pose, side, limits);
                turned = true;
            }
        }
        match gaze.mode {
            Some(mode) => pose_sample.gaze.mode = mode,
            None if turned => pose_sample.gaze.mode = GazeMode::Off,
            None => {}
        }
        if let Some(target) = gaze.target {
            pose_sample.gaze.target = target / scale;
        }
        if let Some(weight) = gaze.weight {
            pose_sample.gaze.weight = weight;
        }
        if let Some(follow) = gaze.head_follow {
            pose_sample.gaze.head_follow = follow;
        }
    }

    let mut reached: Vec<Limb> = Vec::new();
    for (reacher, target) in &key.reach {
        let limb = reacher.limb();
        let pins: Vec<Pin> = Limb::ALL
            .into_iter()
            .filter(|other| *other != limb && pose_sample.pins[other.index()].pin)
            .map(|other| {
                let goal = pose_sample.pins[other.index()].goal;
                Pin {
                    limb: other,
                    position: goal.position * scale,
                    rotation: goal.rotation,
                }
            })
            .collect();
        let handle = match reacher {
            Extremity::Hand(side) => Handle::Hand(*side),
            Extremity::Foot(side) => Handle::Foot(*side),
        };
        let from = handle.point(body, &body.fk(&pose_sample.pose));
        let mut session =
            DragSession::begin(Arc::clone(body), pose_sample.pose, handle, &pins, limits);
        for step in 1..=REACHING {
            let along = from.lerp(*target, step as f32 / REACHING as f32);
            pose_sample.pose = session.update(along - from, DragScope::FullBody);
        }
        reached.push(limb);
    }

    let worlds = body.fk(&pose_sample.pose);
    for (limb, pin) in &key.pins {
        pose_sample.pins[limb.index()] = match pin {
            true => Pinned::new(true, Goal::from_worlds(&worlds, *limb, scale)),
            false => Pinned {
                pin: false,
                ..pose_sample.pins[limb.index()]
            },
        };
    }

    let changed: Vec<Part> = Part::ALL
        .into_iter()
        .filter(|part| differs(&before.values(*part), &pose_sample.values(*part)))
        .collect();
    for limb in Limb::ALL {
        let pinned = &mut pose_sample.pins[limb.index()];
        let moved = reached.contains(&limb) || changed.contains(&Part::from(limb));
        if !pinned.pin || moved {
            pinned.goal = Goal::from_worlds(&worlds, limb, scale);
        }
    }
    let mut parts = changed;
    for limb in reached {
        let part = Part::from(limb);
        if !parts.contains(&part) {
            parts.push(part);
        }
    }
    TakePlayback { parts, clamped }
}

fn place_hips(pose_sample: &mut PoseSample, hips: Hips, scale: f32) {
    if let Some(position) = hips.position {
        pose_sample.pose.root_pos = position / scale;
    }
    let (yaw, pitch, roll) = pose_sample.pose.root_rot.to_euler(EulerRot::YXZ);
    let radians_or = |given: Option<f32>, current: f32| given.map_or(current, f32::to_radians);
    if hips.yaw.is_some() || hips.pitch.is_some() || hips.roll.is_some() {
        pose_sample.pose.root_rot = Quat::from_euler(
            EulerRot::YXZ,
            radians_or(hips.yaw, yaw),
            radians_or(hips.pitch, pitch),
            radians_or(hips.roll, roll),
        );
    }
}
