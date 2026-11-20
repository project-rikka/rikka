use rikka_keyframe::{Ease, Easing, Family};
use rikka_types::Point;

use super::direction::Direction;
use super::letters::{Letters, mixed};
use super::transform::PresetTransform;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LoopKind {
    Shake,
    Pulse,
    Float,
    Spin,
}

impl LoopKind {
    pub const ALL: [LoopKind; 4] = [
        LoopKind::Shake,
        LoopKind::Pulse,
        LoopKind::Float,
        LoopKind::Spin,
    ];

    #[must_use]
    pub fn strength(self) -> f32 {
        match self {
            LoopKind::Shake => 10.0,
            LoopKind::Pulse => 0.1,
            LoopKind::Float => 20.0,
            LoopKind::Spin => 360.0,
        }
    }

    #[must_use]
    pub fn period(self) -> i64 {
        match self {
            LoopKind::Shake => 4,
            LoopKind::Pulse => 30,
            LoopKind::Float | LoopKind::Spin => 60,
        }
    }

    #[must_use]
    pub fn easing(self) -> Easing {
        match self {
            LoopKind::Spin => Easing::LINEAR,
            LoopKind::Shake | LoopKind::Pulse | LoopKind::Float => {
                Easing::new(Family::Sine, Ease::InOut)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoopPreset {
    pub kind: LoopKind,
    pub period: i64,
    pub easing: Easing,
    pub strength: f32,
    pub direction: Direction,
    pub letters: Option<Letters>,
    pub phase: i64,
}

impl LoopPreset {
    #[must_use]
    pub fn new(kind: LoopKind) -> LoopPreset {
        LoopPreset {
            kind,
            period: kind.period(),
            easing: kind.easing(),
            strength: kind.strength(),
            direction: match kind {
                LoopKind::Float => Direction::Up,
                LoopKind::Shake | LoopKind::Pulse | LoopKind::Spin => Direction::Right,
            },
            letters: None,
            phase: 0,
        }
    }

    pub(crate) fn checked(self) -> LoopPreset {
        LoopPreset {
            period: self.period.max(1),
            strength: match self.strength.is_finite() {
                true => self.strength,
                false => self.kind.strength(),
            },
            letters: self.letters.map(Letters::checked),
            ..self
        }
    }

    pub(crate) fn transform_at(&self, at: f64) -> PresetTransform {
        let turns = at / self.period as f64;
        let (round, phase) = (turns.floor(), turns - turns.floor());
        let ease = |progress: f64| self.easing.at(progress) as f32;
        let wave = ease(1.0 - (2.0 * phase - 1.0).abs());
        let strength = self.strength;
        match self.kind {
            LoopKind::Shake => {
                let jolt = |round: f64| {
                    let seed = mixed(round as i64 as u64);
                    let unit = |bits: u64| (bits & 0xffff) as f32 / 32_767.5 - 1.0;
                    Point::new(unit(seed), unit(seed >> 16))
                };
                let (from, to) = (jolt(round), jolt(round + 1.0));
                let along = ease(phase);
                let axes = self.direction.axes();
                let shaken = from + (to - from) * along;
                PresetTransform {
                    offset: Point::new(shaken.x * axes.x, shaken.y * axes.y) * strength,
                    ..PresetTransform::REST
                }
            }
            LoopKind::Pulse => PresetTransform {
                zoom: (1.0 + strength * wave).max(0.0),
                ..PresetTransform::REST
            },
            LoopKind::Float => PresetTransform {
                offset: self.direction.vector() * (strength * (wave - 0.5)),
                ..PresetTransform::REST
            },
            LoopKind::Spin => PresetTransform {
                rotation: strength * (round as f32 + ease(phase)),
                ..PresetTransform::REST
            },
        }
    }
}
