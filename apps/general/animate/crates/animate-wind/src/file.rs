use std::collections::BTreeMap;

use rikka_mmd::VmdCurve;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::key::WindKey;
use crate::motion::WindMotion;
use crate::setting::WindSetting;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct KeyFile {
    setting: String,
    frame: u32,
    value: f32,
    curve: [u8; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindFile {
    levels: BTreeMap<String, f32>,
    keys: Vec<KeyFile>,
}

impl WindFile {
    #[must_use]
    pub fn from_motion(motion: &WindMotion) -> Self {
        Self {
            levels: WindSetting::ALL
                .iter()
                .map(|setting| (setting.id().to_owned(), motion.levels.get(*setting)))
                .collect(),
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
        }
    }

    pub fn motion(&self) -> Result<WindMotion> {
        let mut motion = WindMotion::default();
        for (id, level) in &self.levels {
            let setting =
                WindSetting::named(id).ok_or_else(|| Error::UnknownSetting(id.clone()))?;
            if !setting.fits(*level) {
                return Err(Error::OutOfRange(id.clone()));
            }
            motion.levels.set(setting, *level);
        }
        for key in &self.keys {
            let setting = WindSetting::named(&key.setting)
                .ok_or_else(|| Error::UnknownSetting(key.setting.clone()))?;
            if !setting.fits(key.value) {
                return Err(Error::InvalidKey {
                    setting: key.setting.clone(),
                    frame: key.frame,
                });
            }
            let replaced = motion.tracks_mut().insert(WindKey {
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
        Ok(motion)
    }
}
