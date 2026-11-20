use crate::knob::{Scale, Unit};
use crate::parameters::{Parameters, grouped_knobs};
use rikka_image_edit::{self as imaging, Operation, Primary};
use serde::{Deserialize, Serialize};

const PRIMARIES: [Primary; 3] = [Primary::Red, Primary::Green, Primary::Blue];

const MIX: Scale = Scale::new(-200.0..=200.0).unit(Unit::Percent);

grouped_knobs! {
    for ChannelMixer as KEYS;
    [
        red: "effects.adjust.red",
        green: "effects.adjust.green",
        blue: "effects.adjust.blue",
        gray: "effects.channel_mixer.gray",
    ] {
        red: "effects.adjust.red", MIX;
        green: "effects.adjust.green", MIX;
        blue: "effects.adjust.blue", MIX;
        constant: "effects.channel_mixer.constant", MIX;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Mix {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub constant: f32,
}

impl Mix {
    const fn new(red: f32, green: f32, blue: f32) -> Mix {
        Mix {
            red,
            green,
            blue,
            constant: 0.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChannelMixer {
    pub red: Mix,
    pub green: Mix,
    pub blue: Mix,
    pub monochrome: bool,
    pub gray: Mix,
}

impl Default for ChannelMixer {
    fn default() -> ChannelMixer {
        ChannelMixer {
            red: Mix::new(100.0, 0.0, 0.0),
            green: Mix::new(0.0, 100.0, 0.0),
            blue: Mix::new(0.0, 0.0, 100.0),
            monochrome: false,
            gray: Mix::new(40.0, 40.0, 20.0),
        }
    }
}

impl ChannelMixer {
    pub fn outputs(&self) -> [Mix; 4] {
        [self.red, self.green, self.blue, self.gray]
    }
}

impl Parameters for ChannelMixer {
    fn operation(&self) -> Option<Operation> {
        let mixer = PRIMARIES
            .iter()
            .zip([self.red, self.green, self.blue])
            .fold(imaging::ChannelMixer::new(), |mixer, (primary, mix)| {
                mixer.output(*primary, [mix.red, mix.green, mix.blue], mix.constant)
            });
        let mixer = match self.monochrome {
            true => mixer.monochrome(
                [self.gray.red, self.gray.green, self.gray.blue],
                self.gray.constant,
            ),
            false => mixer,
        };
        Some(mixer.into())
    }
}
