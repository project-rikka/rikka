use std::ops::RangeInclusive;

use rikka_i18n::{Label, tr};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Unit {
    Plain,
    Pixels,
    Percent,
    Degrees,
    Kelvin,
    Seconds,
    Frames,
}

impl Unit {
    pub fn symbol(self) -> Option<Label> {
        match self {
            Unit::Plain => None,
            Unit::Pixels => Some(tr!("effects.unit.pixels")),
            Unit::Percent => Some(Label::from("%")),
            Unit::Degrees => Some(Label::from("°")),
            Unit::Kelvin => Some(Label::from("K")),
            Unit::Seconds => Some(Label::from("s")),
            Unit::Frames => Some(tr!("effects.unit.frames")),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scale {
    pub range: RangeInclusive<f32>,
    pub slide: Option<RangeInclusive<f32>>,
    pub decimals: u8,
    pub unit: Unit,
}

impl Scale {
    #[must_use]
    pub const fn new(range: RangeInclusive<f32>) -> Scale {
        Scale {
            range,
            slide: None,
            decimals: 0,
            unit: Unit::Plain,
        }
    }

    #[must_use]
    pub const fn slide(mut self, slide: RangeInclusive<f32>) -> Scale {
        self.slide = Some(slide);
        self
    }

    #[must_use]
    pub const fn decimals(mut self, decimals: u8) -> Scale {
        self.decimals = decimals;
        self
    }

    #[must_use]
    pub const fn unit(mut self, unit: Unit) -> Scale {
        self.unit = unit;
        self
    }

    pub fn clamp(&self, value: f32) -> f32 {
        value.clamp(*self.range.start(), *self.range.end())
    }

    pub fn track(&self) -> RangeInclusive<f32> {
        self.slide.clone().unwrap_or_else(|| self.range.clone())
    }

    pub fn step(&self) -> f32 {
        10f32.powi(-i32::from(self.decimals))
    }
}

#[derive(Clone, Debug)]
pub struct Knob {
    pub key: &'static str,
    pub label: fn() -> Label,
    pub scale: Scale,
}

impl Knob {
    pub const fn new(key: &'static str, label: fn() -> Label, scale: Scale) -> Knob {
        Knob { key, label, scale }
    }
}

pub(crate) fn grouped(group: Label, part: Label) -> Label {
    tr!(
        "effects.grouped",
        group = group.as_str(),
        part = part.as_str()
    )
}
