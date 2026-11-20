use crate::knob::{Scale, Unit};
use crate::parameters::{Parameters, grouped_knobs};
use rikka_image_edit::{self as imaging, Hues, Operation};
use serde::{Deserialize, Serialize};

const HUES: [Hues; 6] = [
    Hues::Reds,
    Hues::Yellows,
    Hues::Greens,
    Hues::Cyans,
    Hues::Blues,
    Hues::Magentas,
];

const HUE: Scale = Scale::new(-180.0..=180.0).unit(Unit::Degrees);

const COLORIZED_HUE: Scale = Scale::new(0.0..=360.0).unit(Unit::Degrees);

const AMOUNT: Scale = Scale::new(-100.0..=100.0);

const COLORIZED_SATURATION: Scale = Scale::new(0.0..=100.0);

grouped_knobs! {
    for HueSaturation as KEYS;
    [
        master: "effects.hue_saturation.master",
        reds: "effects.hue_saturation.reds",
        yellows: "effects.hue_saturation.yellows",
        greens: "effects.hue_saturation.greens",
        cyans: "effects.hue_saturation.cyans",
        blues: "effects.hue_saturation.blues",
        magentas: "effects.hue_saturation.magentas",
    ] {
        hue: "effects.adjust.hue", HUE;
        saturation: "effects.adjust.saturation", AMOUNT;
        lightness: "effects.adjust.lightness", AMOUNT;
    }
    [colorized: "effects.hue_saturation.colorize"] {
        hue: "effects.adjust.hue", COLORIZED_HUE;
        saturation: "effects.adjust.saturation", COLORIZED_SATURATION;
        lightness: "effects.adjust.lightness", AMOUNT;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Shift {
    hue: f32,
    saturation: f32,
    lightness: f32,
}

impl Shift {
    fn imaged(self) -> imaging::Shift {
        imaging::Shift::new(self.hue, self.saturation, self.lightness)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HueSaturation {
    pub master: Shift,
    pub reds: Shift,
    pub yellows: Shift,
    pub greens: Shift,
    pub cyans: Shift,
    pub blues: Shift,
    pub magentas: Shift,
    pub colorize: bool,
    colorized: Shift,
}

impl Default for HueSaturation {
    fn default() -> HueSaturation {
        HueSaturation {
            master: Shift::default(),
            reds: Shift::default(),
            yellows: Shift::default(),
            greens: Shift::default(),
            cyans: Shift::default(),
            blues: Shift::default(),
            magentas: Shift::default(),
            colorize: false,
            colorized: Shift {
                hue: 0.0,
                saturation: 25.0,
                lightness: 0.0,
            },
        }
    }
}

impl HueSaturation {
    fn ranges(&self) -> [Shift; 7] {
        [
            self.master,
            self.reds,
            self.yellows,
            self.greens,
            self.cyans,
            self.blues,
            self.magentas,
        ]
    }
}

impl Parameters for HueSaturation {
    fn operation(&self) -> Option<Operation> {
        if self.colorize {
            let Shift {
                hue,
                saturation,
                lightness,
            } = self.colorized;
            return Some(
                imaging::HueSaturation::new()
                    .colorize(hue, saturation, lightness)
                    .into(),
            );
        }
        let [master, ranges @ ..] = self.ranges();
        let adjusted = HUES.iter().zip(ranges).fold(
            imaging::HueSaturation::new().master(master.imaged()),
            |adjusted, (hues, shift)| adjusted.hues(*hues, shift.imaged()),
        );
        Some(adjusted.into())
    }
}
