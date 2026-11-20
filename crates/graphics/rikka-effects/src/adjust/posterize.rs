use crate::knob::Scale;
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

knobs! {
    for Posterize;
    LEVELS = "levels" => levels: || tr!("effects.adjust.levels"), Scale::new(2.0..=255.0).slide(2.0..=64.0);
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Posterize {
    levels: f32,
}

impl Default for Posterize {
    fn default() -> Posterize {
        Posterize { levels: 4.0 }
    }
}

impl Parameters for Posterize {
    fn normalize(&mut self, _changed: Option<&str>) {
        self.levels = self.levels.round();
    }

    fn operation(&self) -> Option<Operation> {
        Some(imaging::Posterize::new(self.levels as u16).into())
    }
}
