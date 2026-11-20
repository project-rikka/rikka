use std::sync::Arc;

use animate_assistant::WindEdit;
use animate_cast::Subject;
use animate_edit::{Change, Key};
use animate_stage::Blowing;
use animate_wind::{WindKey, WindSetting};
use rikka_model::mmd::VmdCurve;
use rikka_tool::Reply;
use serde_json::{Map, Value as Json, json};

use super::look::curve_of;
use super::{round, rounded};
use crate::animate::Animate;
use crate::animate::subject_motion::SubjectMotion;

impl Animate {
    pub(super) fn read_wind(&self, frame: Option<u32>) -> Reply {
        let frame = self.frame_or_now(frame);
        let motion = self.wind_motion();
        let levels = motion.levels_at(frame as f32);
        let by_setting = |value: &dyn Fn(WindSetting) -> Json| -> Map<String, Json> {
            WindSetting::ALL
                .iter()
                .map(|setting| (setting.id().to_owned(), value(*setting)))
                .collect()
        };
        let keys: Map<String, Json> = WindSetting::ALL
            .iter()
            .filter(|setting| !motion.tracks().keys(*setting).is_empty())
            .map(|setting| {
                let keys: Vec<Json> = motion
                    .tracks()
                    .keys(setting)
                    .iter()
                    .map(|key| {
                        json!({
                            "frame": key.frame,
                            "value": round(key.value),
                            "curve": curve_of(key.curve),
                        })
                    })
                    .collect();
                (setting.id().to_owned(), Json::Array(keys))
            })
            .collect();
        Reply::json(&json!({
            "frame": frame,
            "values": by_setting(&|setting| round(levels.get(setting))),
            "resting": by_setting(&|setting| round(motion.levels.get(setting))),
            "keys": keys,
            "toward": rounded(levels.direction()),
        }))
    }

    pub(super) fn set_wind(&mut self, edit: &WindEdit) -> Reply {
        if !edit.clear.is_empty() {
            self.clear_settings(Blowing, &edit.clear);
        }
        if !edit.resting.is_empty() {
            self.update_wind(|motion| {
                for (setting, value) in &edit.resting {
                    motion.levels.set(*setting, *value);
                }
            });
        }
        let standing = self.wind_motion();
        let new_keys: Vec<Key> = edit
            .keys
            .iter()
            .flat_map(|key| {
                let standing = &standing;
                key.values.iter().map(move |(setting, value)| {
                    let curve = key.curve.unwrap_or_else(|| {
                        standing
                            .tracks()
                            .key(setting, key.frame)
                            .map_or(VmdCurve::LINEAR, |key| key.curve)
                    });
                    Key::Wind(WindKey {
                        setting: *setting,
                        frame: key.frame,
                        value: *value,
                        curve,
                    })
                })
            })
            .collect();
        if !new_keys.is_empty() {
            self.commit(
                Subject::Wind,
                Change::laying(&SubjectMotion::Wind(Arc::clone(&standing)), new_keys),
            );
        }
        self.read_wind(None)
    }
}
