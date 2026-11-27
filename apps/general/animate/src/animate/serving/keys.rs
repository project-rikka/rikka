use std::sync::Arc;

use animate_assistant::{KeysAction, KeysRequest, KeysSubject};
use animate_cast::Subject;
use animate_edit::{Change, Channel, Key, KeyId};
use animate_motion::Part;
use rikka_tool::Reply;
use rustc_hash::FxHashSet;
use serde_json::{Value as Json, json};

use super::look::{curve_of, target_json};
use super::{ServeError, ToolResult};
use crate::animate::Animate;
use crate::animate::subject_motion::SubjectMotion;

impl Animate {
    pub(super) fn work_keys(&mut self, request: &KeysRequest) -> ToolResult {
        let (subject, subject_motion, keys) = self.keys_of(&request.subject)?;
        let chosen: Vec<Key> = keys
            .into_iter()
            .filter(|key| {
                let id = key.id();
                request.covers(&id.lane, id.frame)
            })
            .collect();
        let ids = || chosen.iter().map(Key::id).collect::<FxHashSet<KeyId>>();
        let change = match request.action {
            KeysAction::List => return Ok(listing(self, &chosen)),
            KeysAction::Delete => Change::deleting(&subject_motion, &ids()),
            KeysAction::Shift(by) => Change::shifting(&subject_motion, &ids(), by),
            KeysAction::Curve(curve) => {
                let channels: &[Channel] = match subject {
                    Subject::Camera => &Channel::OF_CAMERAS,
                    _ => &Channel::OF_VALUES,
                };
                Change::bending(chosen.clone(), channels, curve)
            }
            KeysAction::Tag(tag) => Change::tagging(chosen.clone(), tag),
        };
        let count = chosen.len();
        self.commit(subject, change);
        self.timeline.pick(FxHashSet::default());
        self.refresh();
        Ok(Reply::new(match count {
            0 => "No keys matched; nothing changed.".to_owned(),
            1 => "1 key changed.".to_owned(),
            more => format!("{more} keys changed."),
        }))
    }

    fn keys_of(
        &self,
        whose: &KeysSubject,
    ) -> Result<(Subject, SubjectMotion, Vec<Key>), ServeError> {
        Ok(match whose {
            KeysSubject::Figure(name) => {
                let id = self.figure_called(name)?;
                let take = self.take_of(id);
                let keys = take
                    .human
                    .as_ref()
                    .map(|human| {
                        Part::ALL
                            .into_iter()
                            .flat_map(|part| {
                                human.parts().keys(&part).iter().cloned().map(Key::Part)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                (
                    Subject::Figure(id),
                    SubjectMotion::Take(Arc::clone(&take)),
                    keys,
                )
            }
            KeysSubject::Camera => {
                let film = self.camera_motion().unwrap_or_default();
                let keys = film.keys().iter().copied().map(Key::Camera).collect();
                (Subject::Camera, SubjectMotion::Camera(film), keys)
            }
            KeysSubject::Look => {
                let motion = std::sync::Arc::clone(self.look());
                let keys = motion.tracks().all_keys().cloned().map(Key::Look).collect();
                (Subject::Look, SubjectMotion::Look(motion), keys)
            }
        })
    }
}

fn listing(look: &Animate, chosen: &[Key]) -> Reply {
    let mut sorted: Vec<&Key> = chosen.iter().collect();
    sorted.sort_by_key(|key| key.frame());
    let entries: Vec<Json> = sorted.into_iter().map(|key| key_entry(look, key)).collect();
    Reply::json(&json!({ "keys": entries }))
}

fn key_entry(look: &Animate, key: &Key) -> Json {
    match key {
        Key::Part(key) => json!({
            "lane": key.part.name(),
            "frame": key.frame,
            "tag": key.tag.name(),
        }),
        Key::Camera(key) => json!({
            "lane": "camera",
            "frame": key.frame,
            "curve": curve_of(key.interpolation.rotation),
        }),
        Key::Look(key) => json!({
            "lane": target_json(&key.target),
            "frame": key.frame,
            "value": look.look_key_value(key),
            "curve": curve_of(key.curve),
        }),
        Key::Fluid(key) => json!({
            "lane": key.setting.id(),
            "frame": key.frame,
            "curve": curve_of(key.curve),
        }),
        Key::Wind(key) => json!({
            "lane": key.setting.id(),
            "frame": key.frame,
            "value": key.value,
            "curve": curve_of(key.curve),
        }),
        Key::Prop(key) => json!({
            "lane": key.setting.id(),
            "frame": key.frame,
            "value": key.value,
            "curve": curve_of(key.curve),
        }),
        Key::Anchor(key) => json!({
            "lane": "anchor",
            "frame": key.frame,
            "attachedTo": key.anchor.as_ref().map(|anchor| json!({
                "figure": anchor.figure,
                "bone": anchor.bone,
            })),
        }),
        Key::Bone(frame) => json!({ "lane": frame.bone, "frame": frame.frame }),
        Key::Morph(frame) => json!({ "lane": frame.morph, "frame": frame.frame }),
    }
}
