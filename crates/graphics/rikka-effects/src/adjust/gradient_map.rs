use crate::gradient::Gradient;
use crate::parameters::{Parameters, knobs};
use rikka_i18n::{Label, tr};
use rikka_image_edit::Operation;
use rikka_types::Color;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    BlackWhite,
    Sepia,
    Cyanotype,
    VioletOrange,
    BlueRedYellow,
    TealOrange,
}

impl Preset {
    pub fn name(self) -> Label {
        match self {
            Preset::BlackWhite => tr!("effects.gradient_map.black_white"),
            Preset::Sepia => tr!("effects.gradient_map.sepia"),
            Preset::Cyanotype => tr!("effects.gradient_map.cyanotype"),
            Preset::VioletOrange => tr!("effects.gradient_map.violet_orange"),
            Preset::BlueRedYellow => tr!("effects.gradient_map.blue_red_yellow"),
            Preset::TealOrange => tr!("effects.gradient_map.teal_orange"),
        }
    }

    pub fn gradient(self) -> Gradient {
        let stops: &[(f32, u32)] = match self {
            Preset::BlackWhite => &[(0.0, 0x000000), (1.0, 0xFFFFFF)],
            Preset::Sepia => &[(0.0, 0x1A0F06), (0.5, 0x8C6239), (1.0, 0xF4E7CF)],
            Preset::Cyanotype => &[(0.0, 0x08214A), (1.0, 0xDDEEFF)],
            Preset::VioletOrange => &[(0.0, 0x290A59), (1.0, 0xFF7C00)],
            Preset::BlueRedYellow => &[(0.0, 0x0A00B2), (0.5, 0xFF0000), (1.0, 0xFFFC00)],
            Preset::TealOrange => &[(0.0, 0x0B3A44), (1.0, 0xFFB46B)],
        };
        let stops: Vec<(f32, Color)> = stops
            .iter()
            .map(|&(at, rgb)| (at, Color::hex(rgb)))
            .collect();
        Gradient::from_stops(&stops)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GradientMap {
    pub stops: Gradient,
    pub reversed: bool,
}

pub const PRESETS: [Preset; 6] = [
    Preset::BlackWhite,
    Preset::Sepia,
    Preset::Cyanotype,
    Preset::VioletOrange,
    Preset::BlueRedYellow,
    Preset::TealOrange,
];

impl GradientMap {
    pub fn preset(&self) -> Option<usize> {
        PRESETS
            .iter()
            .position(|preset| preset.gradient().stops() == self.stops.stops())
    }
}

knobs! {
    for GradientMap;
}

impl Parameters for GradientMap {
    fn operation(&self) -> Option<Operation> {
        Some(self.stops.map().reversed(self.reversed).into())
    }
}
