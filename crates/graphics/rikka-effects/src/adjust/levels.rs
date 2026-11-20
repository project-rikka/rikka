use crate::knob::Scale;
use crate::parameters::{Parameters, grouped_knobs};
use rikka_image_edit::{self as imaging, Channel, Histogram, HistogramChannel, Operation};
use serde::{Deserialize, Serialize};

const CHANNELS: [Channel; 4] = [
    Channel::Composite,
    Channel::Red,
    Channel::Green,
    Channel::Blue,
];

pub const MEASURED: [HistogramChannel; 4] = [
    HistogramChannel::Luminosity,
    HistogramChannel::Red,
    HistogramChannel::Green,
    HistogramChannel::Blue,
];

const GAP: f32 = 2.0;

const CLIP: f32 = 0.001;

const LEVEL: Scale = Scale::new(0.0..=255.0);

const GAMMA: Scale = Scale::new(0.01..=9.99).decimals(2);

const NAMES: [&str; 4] = ["rgb", "red", "green", "blue"];

grouped_knobs! {
    for Levels as KEYS;
    [
        rgb: "effects.adjust.rgb",
        red: "effects.adjust.red",
        green: "effects.adjust.green",
        blue: "effects.adjust.blue",
    ] {
        black: "effects.levels.black", LEVEL;
        gamma: "effects.adjust.gamma", GAMMA;
        white: "effects.levels.white", LEVEL;
        out_black: "effects.levels.out_black", LEVEL;
        out_white: "effects.levels.out_white", LEVEL;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Setting {
    pub black: f32,
    pub gamma: f32,
    pub white: f32,
    pub out_black: f32,
    pub out_white: f32,
}

impl Default for Setting {
    fn default() -> Setting {
        Setting {
            black: 0.0,
            gamma: 1.0,
            white: 255.0,
            out_black: 0.0,
            out_white: 255.0,
        }
    }
}

impl Setting {
    fn set_black(&mut self, black: f32) {
        self.black = black.round().clamp(0.0, self.white - GAP);
    }

    fn set_white(&mut self, white: f32) {
        self.white = white.round().clamp(self.black + GAP, 255.0);
    }

    fn set_gamma(&mut self, gamma: f32) {
        self.gamma = GAMMA.clamp((gamma * 100.0).round() / 100.0);
    }

    fn normalize(&mut self, changed: Option<&str>) {
        match changed {
            Some("black") => self.set_black(self.black),
            Some("white") => self.set_white(self.white),
            _ => {
                self.white = self.white.round().clamp(GAP, 255.0);
                self.set_black(self.black);
            }
        }
        self.set_gamma(self.gamma);
        self.out_black = self.out_black.round();
        self.out_white = self.out_white.round();
    }

    pub fn midtone(&self) -> f32 {
        self.black + (self.white - self.black) * 0.5f32.powf(self.gamma)
    }

    fn set_midtone(&mut self, level: f32) {
        let span = (self.white - self.black).max(GAP);
        let along = ((level - self.black) / span).clamp(0.01, 0.99);
        self.set_gamma(along.ln() / 0.5f32.ln());
    }

    fn level(&self) -> imaging::Level {
        imaging::Level::new()
            .input(self.black / 255.0, self.white / 255.0)
            .gamma(self.gamma)
            .output(self.out_black / 255.0, self.out_white / 255.0)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Levels {
    pub rgb: Setting,
    pub red: Setting,
    pub green: Setting,
    pub blue: Setting,
}

impl Levels {
    pub fn auto(&mut self, histogram: &Histogram) {
        for (channel, measured) in MEASURED.into_iter().enumerate() {
            let setting = self.setting(channel);
            *setting = Setting::default();
            if measured == HistogramChannel::Luminosity {
                continue;
            }
            setting.set_white(histogram.percentile(measured, 1.0 - CLIP) * 255.0);
            setting.set_black(histogram.percentile(measured, CLIP) * 255.0);
        }
    }

    pub fn settings(&self) -> [Setting; 4] {
        [self.rgb, self.red, self.green, self.blue]
    }

    fn setting(&mut self, channel: usize) -> &mut Setting {
        match channel {
            0 => &mut self.rgb,
            1 => &mut self.red,
            2 => &mut self.green,
            _ => &mut self.blue,
        }
    }

    pub fn moved_input(&mut self, channel: usize, notch: usize, along: f32) {
        let level = along * 255.0;
        let setting = self.setting(channel);
        match notch {
            0 => setting.set_black(level),
            1 => setting.set_midtone(level),
            _ => setting.set_white(level),
        }
    }

    pub fn moved_output(&mut self, channel: usize, notch: usize, along: f32) {
        let level = (along * 255.0).round();
        let setting = self.setting(channel);
        match notch {
            0 => setting.out_black = level,
            _ => setting.out_white = level,
        }
    }
}

impl Parameters for Levels {
    fn normalize(&mut self, changed: Option<&str>) {
        let changed = changed.and_then(|key| key.split_once('.'));
        for (channel, name) in NAMES.into_iter().enumerate() {
            let part = changed.filter(|(at, _)| *at == name).map(|(_, part)| part);
            self.setting(channel).normalize(part);
        }
    }

    fn operation(&self) -> Option<Operation> {
        let levels = CHANNELS
            .iter()
            .zip(self.settings())
            .fold(imaging::Levels::new(), |levels, (channel, setting)| {
                levels.channel(*channel, setting.level())
            });
        Some(levels.into())
    }
}
