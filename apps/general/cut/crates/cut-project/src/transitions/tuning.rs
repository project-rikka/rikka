use rikka_effects::{Scale, Unit};
use rikka_types::Color;

use super::kind::TransitionKind as Kind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransitionKnob {
    Amount,
    Softness,
    Blur,
    Size,
    Shutter,
    Tint,
}

use TransitionKnob as Knob;

fn clamped(scale: &Scale, value: f32) -> f32 {
    match value.is_finite() {
        true => scale.clamp(value),
        false => *scale.range.start(),
    }
}

impl TransitionKnob {
    #[must_use]
    pub fn scale(self, kind: Kind) -> Option<Scale> {
        let scale = match self {
            Knob::Tint => return None,
            Knob::Softness => Scale::new(0.0..=100.0).unit(Unit::Percent),
            Knob::Blur => Scale::new(0.0..=400.0).unit(Unit::Pixels),
            Knob::Size => Scale::new(10.0..=2_000.0).unit(Unit::Pixels),
            Knob::Shutter => Scale::new(0.0..=200.0).unit(Unit::Percent),
            Knob::Amount => match kind {
                Kind::ZoomIn | Kind::ZoomOut | Kind::Stretch => {
                    Scale::new(110.0..=1_000.0).unit(Unit::Percent)
                }
                Kind::Spin => Scale::new(30.0..=1_080.0).unit(Unit::Degrees),
                Kind::Shake | Kind::ChromaSplit => Scale::new(0.0..=400.0).unit(Unit::Pixels),
                Kind::Pixelate => Scale::new(2.0..=400.0).unit(Unit::Pixels),
                Kind::LiquidWarp => Scale::new(0.0..=800.0).unit(Unit::Pixels),
                Kind::CrossZoom | Kind::LensWarp => Scale::new(0.0..=100.0).unit(Unit::Percent),
                _ => Scale::new(0.0..=200.0).unit(Unit::Percent),
            },
        };
        Some(scale)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tuning {
    pub amount: f32,
    pub softness: f32,
    pub blur: f32,
    pub size: f32,
    pub shutter: f32,
    pub tint: Color,
}

impl Tuning {
    #[must_use]
    pub fn default_for(kind: Kind) -> Tuning {
        Tuning {
            amount: match kind {
                Kind::ZoomIn | Kind::ZoomOut => 300.0,
                Kind::Stretch => 400.0,
                Kind::Spin => 360.0,
                Kind::Shake => 100.0,
                Kind::ChromaSplit => 60.0,
                Kind::Pixelate => 80.0,
                Kind::LiquidWarp => 120.0,
                Kind::CrossZoom => 60.0,
                Kind::LensWarp => 80.0,
                _ => 100.0,
            },
            softness: match kind {
                Kind::NoiseDissolve => 30.0,
                _ => 10.0,
            },
            blur: 60.0,
            size: match kind {
                Kind::LiquidWarp => 240.0,
                _ => 80.0,
            },
            shutter: match kind {
                Kind::Push | Kind::SlideIn | Kind::SlideOut | Kind::CrossZoom | Kind::Shake => 50.0,
                _ => 100.0,
            },
            tint: match kind {
                Kind::Flash => Color::WHITE,
                Kind::LightLeak => Color::rgb(1.0, 0.55, 0.2),
                Kind::FilmBurn => Color::rgb(1.0, 0.45, 0.1),
                _ => Color::BLACK,
            },
        }
    }

    #[must_use]
    pub fn value(&self, knob: Knob) -> Option<f32> {
        match knob {
            Knob::Amount => Some(self.amount),
            Knob::Softness => Some(self.softness),
            Knob::Blur => Some(self.blur),
            Knob::Size => Some(self.size),
            Knob::Shutter => Some(self.shutter),
            Knob::Tint => None,
        }
    }

    #[must_use]
    pub fn with(self, kind: Kind, knob: Knob, value: f32) -> Tuning {
        let Some(scale) = knob.scale(kind) else {
            return self;
        };
        let value = clamped(&scale, value);
        match knob {
            Knob::Amount => Tuning {
                amount: value,
                ..self
            },
            Knob::Softness => Tuning {
                softness: value,
                ..self
            },
            Knob::Blur => Tuning {
                blur: value,
                ..self
            },
            Knob::Size => Tuning {
                size: value,
                ..self
            },
            Knob::Shutter => Tuning {
                shutter: value,
                ..self
            },
            Knob::Tint => self,
        }
    }

    #[must_use]
    pub fn tinted(self, tint: Color) -> Tuning {
        Tuning { tint, ..self }
    }

    pub(crate) fn checked(self, kind: Kind) -> Tuning {
        let fresh = Tuning::default_for(kind);
        let if_used = |knob: Knob, value: f32| match kind.has_knob(knob) {
            true => knob
                .scale(kind)
                .map_or(value, |scale| clamped(&scale, value)),
            false => fresh.value(knob).unwrap_or(value),
        };
        Tuning {
            amount: if_used(Knob::Amount, self.amount),
            softness: if_used(Knob::Softness, self.softness),
            blur: if_used(Knob::Blur, self.blur),
            size: if_used(Knob::Size, self.size),
            shutter: if_used(Knob::Shutter, self.shutter),
            tint: match kind.has_knob(Knob::Tint) {
                true => self.tint,
                false => fresh.tint,
            },
        }
    }
}
