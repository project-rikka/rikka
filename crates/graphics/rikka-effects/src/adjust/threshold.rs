use crate::knob::Scale;
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

pub const LEVEL: Scale = Scale::new(1.0..=255.0);

knobs! {
    for Threshold;
    LEVEL_KNOB = "level" => level: || tr!("effects.threshold.level"), LEVEL;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Threshold {
    pub level: f32,
}

impl Default for Threshold {
    fn default() -> Threshold {
        Threshold { level: 128.0 }
    }
}

impl Parameters for Threshold {
    fn normalize(&mut self, _changed: Option<&str>) {
        self.level = self.level.round();
    }

    fn operation(&self) -> Option<Operation> {
        Some(imaging::Threshold::new(self.level / 255.0).into())
    }
}
