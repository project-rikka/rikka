use std::collections::BTreeMap;
use std::sync::Arc;

use animate_assistant::{dof_name, is_posable, side_name};
use animate_body::{Body, Expression, Faces, HumanBone, Limb, MorphWeights, Side, muscles};
use animate_cast::FigureId;
use animate_motion::{GazeMode, Morphs, Part, PoseSample};
use animate_solve::{EyeDirection, Handle};
use rikka_model::glam::{EulerRot, Quat, Vec3};
use rikka_tool::Reply;
use serde_json::{Map, Value as Json, json};

use super::{ServeError, ToolResult, round, rounded};
use crate::animate::Animate;

const NOTICED: f32 = 0.5;
const SHOWN: f32 = 0.005;

const BONES: [(&str, HumanBone); 2] = [("hips", HumanBone::Hips), ("head", HumanBone::Head)];

const HANDLES: [(&str, Handle); 4] = [
    ("leftHand", Handle::Hand(Side::Left)),
    ("rightHand", Handle::Hand(Side::Right)),
    ("leftFoot", Handle::Foot(Side::Left)),
    ("rightFoot", Handle::Foot(Side::Right)),
];

impl Animate {
    pub(super) fn human(&self, name: &str) -> Result<(FigureId, Arc<Body>), ServeError> {
        let id = self.figure_called(name)?;
        self.cast
            .figure(id)
            .and_then(|figure| figure.body.clone())
            .map(|body| (id, body))
            .ok_or_else(|| ServeError::NotHumanoid {
                name: name.to_owned(),
            })
    }

    pub(super) fn sampled_at(&self, id: FigureId, body: &Body, frame: u32) -> PoseSample {
        self.take_of(id).sample(frame as f32, body.rest_pose())
    }

    pub(super) fn points(&self, body: &Body, pose_sample: &PoseSample, frame: u32) -> Json {
        let eye = self.eye();
        let worlds = body.fk(&pose_sample.still(body, || eye.at(frame)));
        let bones = BONES.map(|(name, bone)| (name, worlds.position(bone)));
        let handles = HANDLES.map(|(name, handle)| (name, handle.point(body, &worlds)));
        let points = bones
            .into_iter()
            .chain(handles)
            .map(|(name, point)| (name.to_owned(), rounded(point)))
            .collect();
        Json::Object(points)
    }

    pub(super) fn faces_of(&self, id: FigureId) -> Arc<Faces> {
        self.cast
            .figure(id)
            .map_or_else(Arc::default, |figure| Arc::clone(&figure.faces))
    }

    pub(super) fn read_figure(&self, name: &str, frame: Option<u32>) -> ToolResult {
        let (id, body) = self.human(name)?;
        let frame = self.frame_or_now(frame);
        let pose_sample = self.sampled_at(id, &body, frame);
        let scale = body.scale();
        let take = self.take_of(id);
        let keys: BTreeMap<&str, Vec<Json>> = take
            .human
            .as_ref()
            .map(|human| {
                Part::ALL
                    .into_iter()
                    .filter(|part| !human.parts().keys(part).is_empty())
                    .map(|part| {
                        let keyed = human
                            .parts()
                            .keys(&part)
                            .iter()
                            .map(|key| json!([key.frame, key.tag.name()]))
                            .collect();
                        (part.name(), keyed)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let bounds = self
            .cast
            .figure(id)
            .map(|figure| figure.model.bounds())
            .map(|bounds| bounds.max.y - bounds.min.y)
            .unwrap_or_default();
        let faces = self.faces_of(id);
        let weights = take.morph_weights(frame as f32);
        let mut reply = json!({
            "figure": name,
            "frame": frame,
            "height": round(bounds),
            "limits": self.limits_of(id),
            "hips": hips(pose_sample.pose.root_pos * scale, pose_sample.pose.root_rot),
            "joints": joints(&pose_sample),
            "expressions": expressions(&faces, &weights),
            "gaze": gaze(&body, &pose_sample, scale, self.limits_of(id)),
            "pins": pins(&pose_sample, scale),
            "points": self.points(&body, &pose_sample, frame),
            "keys": keys,
        });
        if !take.morphs.is_empty() {
            reply["morphs"] = morphs(&weights);
            reply["morphKeys"] = morph_keys(&take.morphs);
        }
        Ok(Reply::json(&reply))
    }

    pub(super) fn limits_of(&self, id: FigureId) -> bool {
        self.take_of(id)
            .human
            .as_ref()
            .is_none_or(|human| human.settings.limits)
    }
}

pub(super) fn hips(position: Vec3, rotation: Quat) -> Json {
    let (yaw, pitch, roll) = rotation.to_euler(EulerRot::YXZ);
    json!({
        "position": rounded(position),
        "yaw": round(yaw.to_degrees()),
        "pitch": round(pitch.to_degrees()),
        "roll": round(roll.to_degrees()),
    })
}

fn joints(pose_sample: &PoseSample) -> Json {
    let mut joints: BTreeMap<&str, Map<String, Json>> = BTreeMap::new();
    for (index, muscle) in muscles().iter().enumerate() {
        if !is_posable(index) {
            continue;
        }
        let degrees = muscle.angle(pose_sample.pose.muscles[index]).to_degrees();
        if degrees.abs() < NOTICED {
            continue;
        }
        joints
            .entry(muscle.bone.name())
            .or_default()
            .insert(dof_name(muscle.dof).to_owned(), round(degrees));
    }
    json!(joints)
}

fn expressions(faces: &Faces, weights: &MorphWeights) -> Json {
    let strengths: BTreeMap<&str, Json> = Expression::ALL
        .into_iter()
        .filter(|expression| faces.has(*expression))
        .map(|expression| (expression.name(), faces.strength(weights, expression)))
        .filter(|(_, weight)| *weight > SHOWN)
        .map(|(name, weight)| (name, round(weight)))
        .collect();
    json!(strengths)
}

fn morphs(weights: &MorphWeights) -> Json {
    let visible: BTreeMap<&str, Json> = weights
        .iter()
        .filter(|(_, weight)| weight.abs() > SHOWN)
        .map(|(morph, weight)| (morph, round(weight)))
        .collect();
    json!(visible)
}

fn morph_keys(morphs: &Morphs) -> Json {
    let keyed: BTreeMap<&str, Vec<u32>> = morphs
        .tracks()
        .map(|(morph, keys)| (morph.as_str(), keys.iter().map(|key| key.frame).collect()))
        .collect();
    json!(keyed)
}

fn gaze(body: &Body, pose_sample: &PoseSample, scale: f32, limits: bool) -> Json {
    let gaze = pose_sample.gaze;
    let mut report = json!({
        "mode": gaze.mode.name(),
        "weight": round(gaze.weight),
        "headFollow": round(gaze.head_follow),
    });
    if gaze.mode == GazeMode::Point {
        report["target"] = rounded(gaze.target * scale);
    }
    let eyes: Map<String, Json> = Side::BOTH
        .into_iter()
        .filter(|side| body.has(side.eye()))
        .map(|side| {
            let eye_direction = EyeDirection::from_pose(&pose_sample.pose, side, limits);
            (
                side_name(side).to_owned(),
                json!([round(eye_direction.across), round(eye_direction.up)]),
            )
        })
        .collect();
    if !eyes.is_empty() {
        report["eyes"] = Json::Object(eyes);
    }
    report
}

fn pins(pose_sample: &PoseSample, scale: f32) -> Json {
    let pinned: BTreeMap<&str, Json> = Limb::ALL
        .into_iter()
        .filter(|limb| pose_sample.pins[limb.index()].pin)
        .map(|limb| {
            let goal = pose_sample.pins[limb.index()].goal;
            (Part::from(limb).name(), rounded(goal.position * scale))
        })
        .collect();
    json!(pinned)
}
