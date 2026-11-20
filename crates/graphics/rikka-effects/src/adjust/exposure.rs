use crate::knob::Scale;
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

knobs! {
    for Exposure;
    EXPOSURE = "exposure" => exposure: || tr!("effects.adjust.exposure"), Scale::new(-20.0..=20.0).slide(-5.0..=5.0).decimals(2);
    OFFSET = "offset" => offset: || tr!("effects.adjust.offset"), Scale::new(-0.5..=0.5).decimals(4);
    GAMMA = "gamma" => gamma: || tr!("effects.adjust.gamma"), Scale::new(0.01..=9.99).slide(0.01..=3.0).decimals(2);
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Exposure {
    exposure: f32,
    offset: f32,
    gamma: f32,
}

impl Default for Exposure {
    fn default() -> Exposure {
        Exposure {
            exposure: 0.0,
            offset: 0.0,
            gamma: 1.0,
        }
    }
}

impl Parameters for Exposure {
    fn operation(&self) -> Option<Operation> {
        Some(
            imaging::Exposure::new(self.exposure)
                .offset(self.offset)
                .gamma(self.gamma)
                .into(),
        )
    }
}
