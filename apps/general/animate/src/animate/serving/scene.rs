use std::collections::BTreeMap;

use animate_cast::{Mode, Prop, Subject};
use animate_motion::Part;
use animate_prop::PropSetting;
use rikka_model::mmd::FRAMES_PER_SECOND;
use rikka_tool::Reply;
use serde_json::{Value as Json, json};

use super::{round, rounded};
use crate::animate::Animate;

impl Animate {
    pub(super) fn describe_scene(&self) -> Reply {
        let transport = &self.transport;
        let handles = self.handles();
        let subject = match self.subject {
            Some(Subject::Figure(id)) => handles
                .iter()
                .find(|(_, candidate)| *candidate == id)
                .map_or(Json::Null, |(handle, _)| Json::from(handle.as_str())),
            Some(Subject::Camera) => Json::from("camera"),
            Some(Subject::Look) => Json::from("look"),
            Some(Subject::Emitter(_)) => Json::from("liquid"),
            Some(Subject::Wind) => Json::from("wind"),
            Some(Subject::Stage) => Json::from("stage"),
            Some(subject @ Subject::Accessory(_)) => self
                .cast
                .prop(subject)
                .map_or(Json::Null, |track| Json::from(track.prop.name.as_str())),
            None => Json::Null,
        };
        let figures: Vec<Json> = handles
            .into_iter()
            .filter_map(|(handle, id)| {
                let figure = self.cast.figure(id)?;
                let bounds = figure.model.bounds();
                let keyed: BTreeMap<&str, Vec<u32>> = figure
                    .take
                    .human
                    .as_ref()
                    .map(|human| {
                        Part::ALL
                            .into_iter()
                            .filter(|part| !human.parts().keys(part).is_empty())
                            .map(|part| {
                                (
                                    part.name(),
                                    human
                                        .parts()
                                        .keys(&part)
                                        .iter()
                                        .map(|key| key.frame)
                                        .collect(),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                Some(json!({
                    "name": handle,
                    "humanoid": figure.is_human(),
                    "height": round(bounds.max.y - bounds.min.y),
                    "keyed": keyed,
                    "otherBoneKeys": figure.take.bones.all_keys().count(),
                }))
            })
            .collect();
        let view = self.stage.lock().current_shot();
        let orbit = view.orbit();
        let target = view.target();
        let camera_keys: Vec<u32> = self
            .camera_motion()
            .map(|film| film.keys().iter().map(|key| key.frame).collect())
            .unwrap_or_default();
        Reply::json(&json!({
            "frame": transport.frame(),
            "fps": FRAMES_PER_SECOND,
            "lastFrame": transport.last_frame(),
            "playing": transport.is_playing(),
            "subject": subject,
            "mode": match self.mode {
                Mode::Easy => "easy",
                Mode::Detail => "detail",
            },
            "physics": self.physics,
            "output": {
                "width": self.output.resolution().width(),
                "height": self.output.resolution().height(),
            },
            "sound": transport.sound_name().map(|name| json!({
                "name": name,
                "seconds": transport.sound_length().map(round),
            })),
            "figures": figures,
            "stage": self.cast.stage().map(|track| self.describe_prop(&track.prop)),
            "accessories": self
                .cast
                .accessories()
                .iter()
                .map(|track| self.describe_prop(&track.prop))
                .collect::<Vec<Json>>(),
            "camera": {
                "keys": camera_keys,
                "view": {
                    "target": rounded(target),
                    "yaw": round(orbit.yaw),
                    "elevation": round(orbit.elevation),
                    "distance": round(orbit.distance),
                    "roll": round(orbit.roll),
                    "fov": round(orbit.fov),
                },
            },
            "look": self.look_summary(),
        }))
    }

    fn describe_prop(&self, prop: &Prop) -> Json {
        let frame = self.transport.frame() as f32;
        let levels = prop.motion.levels_at(frame);
        let keyed: BTreeMap<&str, Vec<u32>> = PropSetting::ALL
            .iter()
            .filter(|setting| !prop.motion.tracks().keys(setting).is_empty())
            .map(|setting| {
                let frames = prop.motion.tracks().keys(setting).iter();
                (setting.id(), frames.map(|key| key.frame).collect())
            })
            .collect();
        let mut described = json!({
            "name": prop.name,
            "position": rounded(levels.position()),
            "rotation": [round(levels.pitch), round(levels.yaw), round(levels.roll)],
            "scale": round(levels.scale),
            "visible": levels.visible,
            "keyed": keyed,
            "boneKeys": prop.take.bones.all_keys().count(),
            "morphKeys": prop.take.morphs.all_keys().count(),
        });
        if prop.kind.attaches() {
            described["attachedTo"] = prop.motion.anchor_at(frame).map_or(
                Json::Null,
                |anchor| json!({ "figure": anchor.figure, "bone": anchor.bone }),
            );
            described["attachmentKeys"] = prop
                .motion
                .anchors()
                .keys(&())
                .iter()
                .map(|key| key.frame)
                .collect();
        }
        described
    }
}
