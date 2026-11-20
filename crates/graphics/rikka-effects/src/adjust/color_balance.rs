use crate::knob::Scale;
use crate::parameters::{Parameters, grouped_knobs};
use rikka_i18n::{Label, tr};
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

const BALANCE: Scale = Scale::new(-100.0..=100.0);

grouped_knobs! {
    for ColorBalance as KEYS;
    [
        shadows: "effects.adjust.shadows",
        midtones: "effects.adjust.midtones",
        highlights: "effects.adjust.highlights",
    ] {
        cyan_red: "effects.color_balance.cyan_red", BALANCE;
        magenta_green: "effects.color_balance.magenta_green", BALANCE;
        yellow_blue: "effects.color_balance.yellow_blue", BALANCE;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Balance {
    cyan_red: f32,
    magenta_green: f32,
    yellow_blue: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorBalance {
    pub shadows: Balance,
    pub midtones: Balance,
    pub highlights: Balance,
    pub preserve_luminosity: bool,
}

impl Default for ColorBalance {
    fn default() -> ColorBalance {
        ColorBalance {
            shadows: Balance::default(),
            midtones: Balance::default(),
            highlights: Balance::default(),
            preserve_luminosity: true,
        }
    }
}

pub fn axes() -> [(Label, Label); 3] {
    [
        (tr!("effects.adjust.cyan"), tr!("effects.adjust.red")),
        (tr!("effects.adjust.magenta"), tr!("effects.adjust.green")),
        (tr!("effects.adjust.yellow"), tr!("effects.adjust.blue")),
    ]
}

impl Parameters for ColorBalance {
    fn operation(&self) -> Option<Operation> {
        let axes =
            |balance: Balance| (balance.cyan_red, balance.magenta_green, balance.yellow_blue);
        let (shadows, midtones, highlights) = (
            axes(self.shadows),
            axes(self.midtones),
            axes(self.highlights),
        );
        Some(
            imaging::ColorBalance::new()
                .shadows(shadows.0, shadows.1, shadows.2)
                .midtones(midtones.0, midtones.1, midtones.2)
                .highlights(highlights.0, highlights.1, highlights.2)
                .preserve_luminosity(self.preserve_luminosity)
                .into(),
        )
    }
}
