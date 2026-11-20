use crate::knob::Scale;
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

knobs! {
    for Vignette;
    AMOUNT = "amount" => amount: || tr!("effects.adjust.amount"), Scale::new(-100.0..=100.0);
    MIDPOINT = "midpoint" => midpoint: || tr!("effects.vignette.midpoint"), crate::filter::PERCENT;
    ROUNDNESS = "roundness" => roundness: || tr!("effects.vignette.roundness"), Scale::new(-100.0..=100.0);
    FEATHER = "feather" => feather: || tr!("effects.vignette.feather"), crate::filter::PERCENT;
    HIGHLIGHTS = "highlights" => highlights: || tr!("effects.adjust.highlights"), crate::filter::PERCENT;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Vignette {
    amount: f32,
    midpoint: f32,
    roundness: f32,
    feather: f32,
    highlights: f32,
}

impl Default for Vignette {
    fn default() -> Vignette {
        Vignette {
            amount: -50.0,
            midpoint: 50.0,
            roundness: 0.0,
            feather: 50.0,
            highlights: 0.0,
        }
    }
}

impl Parameters for Vignette {
    fn operation(&self) -> Option<Operation> {
        Some(
            imaging::Vignette::new(self.amount)
                .midpoint(self.midpoint)
                .roundness(self.roundness)
                .feather(self.feather)
                .highlights(self.highlights)
                .into(),
        )
    }
}
