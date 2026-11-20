use crate::curve::Curve;
use crate::parameters::{Parameters, knobs};
use rikka_i18n::{Label, tr};
use rikka_image_edit::{self as imaging, Channel, Operation};
use serde::{Deserialize, Serialize};

const CHANNELS: [Channel; 4] = [
    Channel::Composite,
    Channel::Red,
    Channel::Green,
    Channel::Blue,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Linear,
    IncreaseContrast,
    DecreaseContrast,
    Lighter,
    Darker,
    Negative,
}

impl Preset {
    pub fn name(self) -> Label {
        match self {
            Preset::Linear => tr!("effects.curves.linear"),
            Preset::IncreaseContrast => tr!("effects.curves.increase_contrast"),
            Preset::DecreaseContrast => tr!("effects.curves.decrease_contrast"),
            Preset::Lighter => tr!("effects.curves.lighter"),
            Preset::Darker => tr!("effects.curves.darker"),
            Preset::Negative => tr!("effects.curves.negative"),
        }
    }

    pub fn curve(self) -> Curve {
        let levels: &[(f32, f32)] = match self {
            Preset::Linear => &[(0.0, 0.0), (255.0, 255.0)],
            Preset::IncreaseContrast => &[(0.0, 0.0), (64.0, 47.0), (190.0, 208.0), (255.0, 255.0)],
            Preset::DecreaseContrast => &[(0.0, 0.0), (64.0, 82.0), (190.0, 173.0), (255.0, 255.0)],
            Preset::Lighter => &[(0.0, 0.0), (114.0, 150.0), (255.0, 255.0)],
            Preset::Darker => &[(0.0, 0.0), (140.0, 104.0), (255.0, 255.0)],
            Preset::Negative => &[(0.0, 255.0), (255.0, 0.0)],
        };
        Curve::from_points(levels.iter().map(|&(x, y)| (x / 255.0, y / 255.0)))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Curves {
    rgb: Curve,
    red: Curve,
    green: Curve,
    blue: Curve,
}

pub const PRESETS: [Preset; 6] = [
    Preset::Linear,
    Preset::IncreaseContrast,
    Preset::DecreaseContrast,
    Preset::Lighter,
    Preset::Darker,
    Preset::Negative,
];

impl Curves {
    pub fn curves(&self) -> [&Curve; 4] {
        [&self.rgb, &self.red, &self.green, &self.blue]
    }

    pub fn curve(&mut self, channel: usize) -> &mut Curve {
        match channel {
            0 => &mut self.rgb,
            1 => &mut self.red,
            2 => &mut self.green,
            _ => &mut self.blue,
        }
    }

    pub fn preset(&self, channel: usize) -> Option<usize> {
        let curve = self.curves()[channel];
        PRESETS.iter().position(|preset| preset.curve() == *curve)
    }
}

knobs! {
    for Curves;
}

impl Parameters for Curves {
    fn operation(&self) -> Option<Operation> {
        let curves = CHANNELS.iter().zip(self.curves()).fold(
            imaging::Curves::new(),
            |curves, (channel, curve)| match curve.is_identity() {
                true => curves,
                false => curves.channel(*channel, curve.points().iter().copied()),
            },
        );
        Some(curves.into())
    }
}
