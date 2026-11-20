use crate::hex;
use crate::knob::{Scale, Unit};
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Hues, Operation};
use rikka_types::Color;
use serde::{Deserialize, Serialize};

const HUES: [Hues; 6] = [
    Hues::Reds,
    Hues::Yellows,
    Hues::Greens,
    Hues::Cyans,
    Hues::Blues,
    Hues::Magentas,
];

const WEIGHT: Scale = Scale::new(-200.0..=300.0).unit(Unit::Percent);

const TINT: Color = Color::rgb(225.0 / 255.0, 211.0 / 255.0, 179.0 / 255.0);

pub const PRESETS: [[f32; 6]; 5] = [
    [40.0, 60.0, 40.0, 60.0, 20.0, 80.0],
    [120.0, 110.0, -10.0, -50.0, -50.0, 120.0],
    [120.0, 110.0, 40.0, -30.0, 0.0, 70.0],
    [-40.0, 235.0, 144.0, -68.0, -3.0, -107.0],
    [100.0, 100.0, 100.0, 100.0, 100.0, 100.0],
];

knobs! {
    for BlackWhite;
    REDS = "reds" => reds: || tr!("effects.hue_saturation.reds"), WEIGHT;
    YELLOWS = "yellows" => yellows: || tr!("effects.hue_saturation.yellows"), WEIGHT;
    GREENS = "greens" => greens: || tr!("effects.hue_saturation.greens"), WEIGHT;
    CYANS = "cyans" => cyans: || tr!("effects.hue_saturation.cyans"), WEIGHT;
    BLUES = "blues" => blues: || tr!("effects.hue_saturation.blues"), WEIGHT;
    MAGENTAS = "magentas" => magentas: || tr!("effects.hue_saturation.magentas"), WEIGHT;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BlackWhite {
    reds: f32,
    yellows: f32,
    greens: f32,
    cyans: f32,
    blues: f32,
    magentas: f32,
    pub tinted: bool,
    #[serde(with = "hex")]
    pub tint: Color,
}

impl Default for BlackWhite {
    fn default() -> BlackWhite {
        let mut toned = BlackWhite {
            reds: 0.0,
            yellows: 0.0,
            greens: 0.0,
            cyans: 0.0,
            blues: 0.0,
            magentas: 0.0,
            tinted: false,
            tint: TINT,
        };
        toned.set_weights(PRESETS[0]);
        toned
    }
}

impl BlackWhite {
    pub fn weights(&self) -> [f32; 6] {
        [
            self.reds,
            self.yellows,
            self.greens,
            self.cyans,
            self.blues,
            self.magentas,
        ]
    }

    pub fn set_weights(&mut self, [reds, yellows, greens, cyans, blues, magentas]: [f32; 6]) {
        self.reds = reds;
        self.yellows = yellows;
        self.greens = greens;
        self.cyans = cyans;
        self.blues = blues;
        self.magentas = magentas;
    }
}

impl Parameters for BlackWhite {
    fn operation(&self) -> Option<Operation> {
        let toned = HUES
            .iter()
            .zip(self.weights())
            .fold(imaging::BlackWhite::new(), |toned, (hues, weight)| {
                toned.weight(*hues, weight)
            });
        let toned = match self.tinted {
            true => toned.tint(self.tint),
            false => toned,
        };
        Some(toned.into())
    }
}
