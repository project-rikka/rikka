use super::PERCENT;
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Lumas, Operation};
use serde::{Deserialize, Serialize};

knobs! {
    for LumaKey;
    THRESHOLD = "threshold" => threshold: || tr!("effects.adjust.threshold"), PERCENT;
    SOFTNESS = "softness" => softness: || tr!("effects.key.softness"), PERCENT;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Keep {
    #[default]
    Bright,
    Dark,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LumaKey {
    threshold: f32,
    softness: f32,
    pub keeps: Keep,
}

impl Default for LumaKey {
    fn default() -> LumaKey {
        LumaKey {
            threshold: 50.0,
            softness: 10.0,
            keeps: Keep::default(),
        }
    }
}

impl Parameters for LumaKey {
    fn operation(&self) -> Option<Operation> {
        let keeps = match self.keeps {
            Keep::Bright => Lumas::Bright,
            Keep::Dark => Lumas::Dark,
        };
        Some(
            imaging::LumaKey::new(self.threshold)
                .softness(self.softness)
                .keeps(keeps)
                .into(),
        )
    }
}
