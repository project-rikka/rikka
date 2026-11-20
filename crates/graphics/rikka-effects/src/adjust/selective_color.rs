use crate::knob::{Scale, Unit};
use crate::parameters::{Parameters, grouped_knobs};
use rikka_image_edit::{self as imaging, Colors, Operation};
use serde::{Deserialize, Serialize};

const COLORS: [Colors; 9] = [
    Colors::Reds,
    Colors::Yellows,
    Colors::Greens,
    Colors::Cyans,
    Colors::Blues,
    Colors::Magentas,
    Colors::Whites,
    Colors::Neutrals,
    Colors::Blacks,
];

const PLATE: Scale = Scale::new(-100.0..=100.0).unit(Unit::Percent);

grouped_knobs! {
    for SelectiveColor as KEYS;
    [
        reds: "effects.hue_saturation.reds",
        yellows: "effects.hue_saturation.yellows",
        greens: "effects.hue_saturation.greens",
        cyans: "effects.hue_saturation.cyans",
        blues: "effects.hue_saturation.blues",
        magentas: "effects.hue_saturation.magentas",
        whites: "effects.selective_color.whites",
        neutrals: "effects.selective_color.neutrals",
        blacks: "effects.selective_color.blacks",
    ] {
        cyan: "effects.adjust.cyan", PLATE;
        magenta: "effects.adjust.magenta", PLATE;
        yellow: "effects.adjust.yellow", PLATE;
        black: "effects.adjust.black", PLATE;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Plate {
    cyan: f32,
    magenta: f32,
    yellow: f32,
    black: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SelectiveColor {
    pub reds: Plate,
    pub yellows: Plate,
    pub greens: Plate,
    pub cyans: Plate,
    pub blues: Plate,
    pub magentas: Plate,
    pub whites: Plate,
    pub neutrals: Plate,
    pub blacks: Plate,
    pub absolute: bool,
}

impl SelectiveColor {
    fn plates(&self) -> [Plate; 9] {
        [
            self.reds,
            self.yellows,
            self.greens,
            self.cyans,
            self.blues,
            self.magentas,
            self.whites,
            self.neutrals,
            self.blacks,
        ]
    }
}

impl Parameters for SelectiveColor {
    fn operation(&self) -> Option<Operation> {
        let adjusted = COLORS.iter().zip(self.plates()).fold(
            imaging::SelectiveColor::new(),
            |adjusted, (colors, plate)| {
                adjusted.colors(
                    *colors,
                    plate.cyan,
                    plate.magenta,
                    plate.yellow,
                    plate.black,
                )
            },
        );
        Some(adjusted.absolute(self.absolute).into())
    }
}
