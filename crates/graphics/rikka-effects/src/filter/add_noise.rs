use crate::knob::{Scale, Unit};
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Distribution, Operation};
use serde::{Deserialize, Serialize};

knobs! {
    for AddNoise;
    AMOUNT = "amount" => amount: || tr!("effects.adjust.amount"), Scale::new(0.1..=400.0)
            .slide(0.1..=100.0)
            .decimals(1)
            .unit(Unit::Percent);
    SEED = "seed" => seed: || tr!("effects.add_noise.seed"), Scale::new(0.0..=65535.0);
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Spread {
    #[default]
    Uniform,
    Gaussian,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AddNoise {
    amount: f32,
    pub distribution: Spread,
    pub monochromatic: bool,
    pub seed: f32,
}

impl Default for AddNoise {
    fn default() -> AddNoise {
        AddNoise {
            amount: 12.5,
            distribution: Spread::default(),
            monochromatic: false,
            seed: 0.0,
        }
    }
}

impl Parameters for AddNoise {
    fn normalize(&mut self, _changed: Option<&str>) {
        self.seed = self.seed.round();
    }

    fn operation(&self) -> Option<Operation> {
        let distribution = match self.distribution {
            Spread::Uniform => Distribution::Uniform,
            Spread::Gaussian => Distribution::Gaussian,
        };
        Some(
            imaging::AddNoise::new(self.amount)
                .distribution(distribution)
                .monochromatic(self.monochromatic)
                .seed(self.seed as u32)
                .into(),
        )
    }
}
