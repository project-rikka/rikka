use std::collections::BTreeMap;

use rikka_mmd::VmdCurve;
use serde::{Deserialize, Serialize};

use crate::anchor::{Anchor, AnchorKey};
use crate::error::{Error, Result};
use crate::key::PropKey;
use crate::motion::PropMotion;
use crate::setting::PropSetting;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct KeyFile {
    setting: String,
    frame: u32,
    value: f32,
    curve: [u8; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct AnchorFile {
    frame: u32,
    anchor: Option<Anchor>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PropFile {
    levels: BTreeMap<String, f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    anchor: Option<Anchor>,
    keys: Vec<KeyFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    anchors: Vec<AnchorFile>,
}

impl PropFile {
    #[must_use]
    pub fn from_motion(motion: &PropMotion) -> Self {
        Self {
            levels: PropSetting::ALL
                .iter()
                .map(|setting| (setting.id().to_owned(), motion.levels.get(*setting)))
                .collect(),
            anchor: motion.anchor.clone(),
            keys: motion
                .tracks()
                .all_keys()
                .map(|key| KeyFile {
                    setting: key.setting.id().to_owned(),
                    frame: key.frame,
                    value: key.value,
                    curve: key.curve.to_bytes(),
                })
                .collect(),
            anchors: motion
                .anchors()
                .all_keys()
                .map(|key| AnchorFile {
                    frame: key.frame,
                    anchor: key.anchor.clone(),
                })
                .collect(),
        }
    }

    pub fn motion(&self) -> Result<PropMotion> {
        let mut motion = PropMotion::default();
        for (id, level) in &self.levels {
            let setting =
                PropSetting::named(id).ok_or_else(|| Error::UnknownSetting(id.clone()))?;
            if !setting.fits(*level) {
                return Err(Error::OutOfRange(id.clone()));
            }
            motion.levels.set(setting, *level);
        }
        if let Some(anchor) = &self.anchor {
            if !anchor.is_valid() {
                return Err(Error::InvalidAnchor(0));
            }
            motion.anchor = Some(anchor.clone());
        }
        for key in &self.keys {
            let setting = PropSetting::named(&key.setting)
                .ok_or_else(|| Error::UnknownSetting(key.setting.clone()))?;
            if !setting.fits(key.value) {
                return Err(Error::InvalidKey {
                    setting: key.setting.clone(),
                    frame: key.frame,
                });
            }
            let replaced = motion.tracks_mut().insert(PropKey {
                setting,
                frame: key.frame,
                value: key.value,
                curve: VmdCurve::from_bytes(key.curve),
            });
            if replaced.is_some() {
                return Err(Error::DuplicateKey {
                    setting: key.setting.clone(),
                    frame: key.frame,
                });
            }
        }
        for key in &self.anchors {
            if key.anchor.as_ref().is_some_and(|anchor| !anchor.is_valid()) {
                return Err(Error::InvalidAnchor(key.frame));
            }
            let replaced = motion.anchors_mut().insert(AnchorKey {
                frame: key.frame,
                anchor: key.anchor.clone(),
            });
            if replaced.is_some() {
                return Err(Error::DuplicateAnchor(key.frame));
            }
        }
        Ok(motion)
    }
}
