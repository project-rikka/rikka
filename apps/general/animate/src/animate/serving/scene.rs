use std::collections::BTreeMap;

use animate_cast::{Mode, Subject};
use animate_motion::Part;
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
}
