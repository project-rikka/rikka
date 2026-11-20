mod parts;

use std::collections::BTreeMap;

use rikka_mmd::VmdCurve;
use serde::{Deserialize, Serialize};

use crate::aim::Aim;
use crate::emitter::{Emitter, EmitterId};
use crate::error::{Error, Result};
use crate::key::EmitterKey;
use crate::setting::EmitterSetting;
use crate::value::EmitterValue;

use parts::{FormulaFile, PourFile, ShapeFile, SpotFile, ValueFile};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct KeyFile {
    setting: String,
    frame: u32,
    value: ValueFile,
    curve: [u8; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EmitterFile {
    name: String,
    shape: ShapeFile,
    aim: String,
    launch: SpotFile,
    target: SpotFile,
    levels: BTreeMap<String, f32>,
    liquid: FormulaFile,
    pour: PourFile,
    keys: Vec<KeyFile>,
}

impl EmitterFile {
    #[must_use]
    pub fn from_emitter(emitter: &Emitter) -> Self {
        Self {
            name: emitter.name.clone(),
            shape: ShapeFile::from_shape(&emitter.shape),
            aim: emitter.aim.id().to_owned(),
            launch: SpotFile::from_spot(&emitter.launch),
            target: SpotFile::from_spot(&emitter.target),
            levels: EmitterSetting::ALL
                .iter()
                .filter_map(|setting| {
                    let level = emitter.levels.get(*setting)?;
                    Some((setting.id().to_owned(), level))
                })
                .collect(),
            liquid: FormulaFile::from_formula(&emitter.liquid),
            pour: PourFile::from_pour(emitter.pour),
            keys: emitter
                .tracks()
                .all_keys()
                .map(|key| KeyFile {
                    setting: key.setting.id().to_owned(),
                    frame: key.frame,
                    value: ValueFile::from_value(key.value),
                    curve: key.curve.to_bytes(),
                })
                .collect(),
        }
    }

    pub fn emitter(&self, id: EmitterId) -> Result<Emitter> {
        let mut emitter = Emitter::new(id, self.name.clone(), self.shape.shape()?);
        emitter.aim = Aim::named(&self.aim).ok_or_else(|| Error::UnknownAim(self.aim.clone()))?;
        emitter.launch = self.launch.spot();
        emitter.target = self.target.spot();
        emitter.liquid = self.liquid.formula()?;
        emitter.pour = self.pour.pour()?;
        for (id, level) in &self.levels {
            let setting =
                EmitterSetting::named(id).ok_or_else(|| Error::UnknownSetting(id.clone()))?;
            let fits = emitter.levels.get(setting).is_some()
                && setting.kind().fits(EmitterValue::Number(*level));
            if !fits {
                return Err(Error::OutOfRange { name: id.clone() });
            }
            emitter.levels.set(setting, *level);
        }
        for key in &self.keys {
            let setting = EmitterSetting::named(&key.setting)
                .ok_or_else(|| Error::UnknownSetting(key.setting.clone()))?;
            let value = key.value.value();
            if !setting.kind().fits(value) {
                return Err(Error::InvalidKey {
                    setting: key.setting.clone(),
                    frame: key.frame,
                });
            }
            let replaced = emitter.tracks_mut().insert(EmitterKey {
                setting,
                frame: key.frame,
                value,
                curve: VmdCurve::from_bytes(key.curve),
            });
            if replaced.is_some() {
                return Err(Error::DuplicateKey {
                    setting: key.setting.clone(),
                    frame: key.frame,
                });
            }
        }
        Ok(emitter)
    }
}
