use std::f64::consts::FRAC_PI_2;
use std::ops::RangeInclusive;

use crate::animation::Value;
use crate::placement::Placement;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mirror {
    pub horizontal: bool,
    pub vertical: bool,
}

impl Mirror {
    #[must_use]
    pub fn signs(self) -> (f32, f32) {
        let sign = |flipped: bool| if flipped { -1.0 } else { 1.0 };
        (sign(self.horizontal), sign(self.vertical))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MixField {
    Volume,
    Pan,
}

impl MixField {
    pub const ALL: [MixField; 2] = [MixField::Volume, MixField::Pan];

    #[must_use]
    pub fn range(self) -> RangeInclusive<f32> {
        match self {
            MixField::Volume => Mix::VOLUMES,
            MixField::Pan => Mix::PANS,
        }
    }

    fn rest(self) -> f32 {
        match self {
            MixField::Volume => 1.0,
            MixField::Pan => 0.0,
        }
    }

    pub(crate) fn clamp_value(self, value: Value) -> Value {
        match value {
            Value::Scalar(value) => Value::Scalar(clamp_or(value, self.range(), self.rest())),
            other => other,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FadeShape {
    Linear,
    #[default]
    EqualPower,
    SCurve,
    Exponential,
}

impl FadeShape {
    pub const ALL: [FadeShape; 4] = [
        FadeShape::Linear,
        FadeShape::EqualPower,
        FadeShape::SCurve,
        FadeShape::Exponential,
    ];

    #[must_use]
    pub fn gain(self, progress: f64) -> f64 {
        let progress = progress.clamp(0.0, 1.0);
        match self {
            FadeShape::Linear => progress,
            FadeShape::EqualPower => (progress * FRAC_PI_2).sin(),
            FadeShape::SCurve => progress * progress * (3.0 - 2.0 * progress),
            FadeShape::Exponential => progress * progress,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Fade {
    pub length: i64,
    pub shape: FadeShape,
}

impl Fade {
    #[must_use]
    pub fn is_on(self) -> bool {
        self.length > 0
    }

    fn gain(self, from_edge: f64) -> f64 {
        match self.is_on() {
            true => self.shape.gain(from_edge / self.length as f64),
            false => 1.0,
        }
    }

    pub(crate) fn longest(self, length: i64) -> Fade {
        Fade {
            length: self.length.clamp(0, length.max(0)),
            ..self
        }
    }

    pub(crate) fn retimed(self, frames: impl Fn(i64) -> i64) -> Fade {
        Fade {
            length: frames(self.length),
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mix {
    pub volume: f32,
    pub pan: f32,
    pub muted: bool,
    pub fade_in: Fade,
    pub fade_out: Fade,
}

impl Default for Mix {
    fn default() -> Mix {
        Mix {
            volume: 1.0,
            pan: 0.0,
            muted: false,
            fade_in: Fade::default(),
            fade_out: Fade::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Level {
    pub gain: f32,
    pub pan: f32,
}

impl Mix {
    pub const VOLUMES: RangeInclusive<f32> = 0.0..=2.0;
    pub const PANS: RangeInclusive<f32> = -1.0..=1.0;

    #[must_use]
    pub fn gain(&self) -> f32 {
        match self.muted {
            true => 0.0,
            false => self.volume,
        }
    }

    #[must_use]
    pub fn value(&self, field: MixField) -> f32 {
        match field {
            MixField::Volume => self.volume,
            MixField::Pan => self.pan,
        }
    }

    #[must_use]
    pub fn with(self, field: MixField, value: f32) -> Mix {
        match field {
            MixField::Volume => Mix {
                volume: value,
                ..self
            },
            MixField::Pan => Mix { pan: value, ..self },
        }
    }

    #[must_use]
    pub fn level(&self, local: f64, length: i64) -> Level {
        let local = local.clamp(0.0, length as f64);
        let faded = self.fade_in.gain(local) * self.fade_out.gain(length as f64 - local);
        Level {
            gain: self.gain() * faded as f32,
            pan: self.pan,
        }
    }

    #[must_use]
    pub fn checked(self) -> Mix {
        let fade = |fade: Fade| Fade {
            length: fade.length.max(0),
            ..fade
        };
        Mix {
            volume: clamp_or(self.volume, Mix::VOLUMES, 1.0),
            pan: clamp_or(self.pan, Mix::PANS, 0.0),
            muted: self.muted,
            fade_in: fade(self.fade_in),
            fade_out: fade(self.fade_out),
        }
    }

    pub(crate) fn fitted(self, length: i64) -> Mix {
        let fade_in = self.fade_in.longest(length);
        Mix {
            fade_in,
            fade_out: self.fade_out.longest(length - fade_in.length),
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Treatment {
    pub placement: Placement,
    pub radius: f32,
    pub mirror: Mirror,
    pub speed: f32,
    pub mix: Mix,
}

impl Default for Treatment {
    fn default() -> Treatment {
        Treatment {
            placement: Placement::default(),
            radius: 0.0,
            mirror: Mirror::default(),
            speed: 1.0,
            mix: Mix::default(),
        }
    }
}

impl Treatment {
    pub const SPEEDS: RangeInclusive<f32> = 0.1..=4.0;

    #[must_use]
    pub fn checked(self) -> Treatment {
        Treatment {
            placement: self.placement.checked(),
            radius: match self.radius.is_finite() {
                true => self.radius.max(0.0),
                false => 0.0,
            },
            mirror: self.mirror,
            speed: clamp_or(self.speed, Treatment::SPEEDS, 1.0),
            mix: self.mix.checked(),
        }
    }
}

fn clamp_or(value: f32, range: RangeInclusive<f32>, otherwise: f32) -> f32 {
    match value.is_finite() {
        true => value.clamp(*range.start(), *range.end()),
        false => otherwise,
    }
}
