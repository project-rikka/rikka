use rikka_keyframe::{Ease, Easing, Family};

use super::direction::Direction;
use super::letters::Letters;
use super::transform::{PresetTransform, Wipe};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AnimationPresetKind {
    Fade,
    Slide,
    Pop,
    Zoom,
    Spin,
    Wipe,
    Blur,
    Typewriter,
}

impl AnimationPresetKind {
    pub const ALL: [AnimationPresetKind; 8] = [
        AnimationPresetKind::Fade,
        AnimationPresetKind::Slide,
        AnimationPresetKind::Pop,
        AnimationPresetKind::Zoom,
        AnimationPresetKind::Spin,
        AnimationPresetKind::Wipe,
        AnimationPresetKind::Blur,
        AnimationPresetKind::Typewriter,
    ];

    #[must_use]
    pub fn strength(self) -> f32 {
        match self {
            AnimationPresetKind::Fade | AnimationPresetKind::Pop | AnimationPresetKind::Zoom => 1.0,
            AnimationPresetKind::Wipe | AnimationPresetKind::Typewriter => 1.0,
            AnimationPresetKind::Slide => 200.0,
            AnimationPresetKind::Spin => 180.0,
            AnimationPresetKind::Blur => 30.0,
        }
    }

    #[must_use]
    pub fn easing(self, ease: Ease) -> Easing {
        match self {
            AnimationPresetKind::Fade | AnimationPresetKind::Typewriter => Easing::LINEAR,
            AnimationPresetKind::Pop => Easing::new(Family::Back, ease),
            AnimationPresetKind::Wipe => Easing::new(Family::Cubic, Ease::InOut),
            AnimationPresetKind::Slide
            | AnimationPresetKind::Zoom
            | AnimationPresetKind::Spin
            | AnimationPresetKind::Blur => Easing::new(Family::Cubic, ease),
        }
    }

    pub(crate) fn transform_at(
        self,
        rest: f32,
        strength: f32,
        direction: Direction,
    ) -> PresetTransform {
        let away = 1.0 - rest;
        let opacity = rest.clamp(0.0, 1.0);
        match self {
            AnimationPresetKind::Fade => PresetTransform {
                opacity: (1.0 - strength * away).clamp(0.0, 1.0),
                ..PresetTransform::REST
            },
            AnimationPresetKind::Slide => PresetTransform {
                offset: direction.vector() * (strength * away),
                ..PresetTransform::REST
            },
            AnimationPresetKind::Pop => PresetTransform {
                zoom: (1.0 - strength * away).max(0.0),
                ..PresetTransform::REST
            },
            AnimationPresetKind::Zoom => PresetTransform {
                zoom: (1.0 + strength * away).max(0.0),
                opacity,
                ..PresetTransform::REST
            },
            AnimationPresetKind::Spin => PresetTransform {
                rotation: strength * away,
                opacity,
                ..PresetTransform::REST
            },
            AnimationPresetKind::Wipe => PresetTransform {
                wipe: (opacity < 1.0).then_some(Wipe {
                    direction,
                    progress: opacity,
                }),
                ..PresetTransform::REST
            },
            AnimationPresetKind::Blur => PresetTransform {
                blur: (strength * away).max(0.0),
                opacity,
                ..PresetTransform::REST
            },
            AnimationPresetKind::Typewriter => PresetTransform {
                reveal: opacity,
                ..PresetTransform::REST
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimationPreset {
    pub kind: AnimationPresetKind,
    pub length: i64,
    pub easing: Easing,
    pub strength: f32,
    pub direction: Direction,
    pub letters: Option<Letters>,
}

impl AnimationPreset {
    pub const LENGTH: i64 = 15;

    #[must_use]
    pub fn new_in(kind: AnimationPresetKind) -> AnimationPreset {
        AnimationPreset::new(kind, Ease::Out)
    }

    #[must_use]
    pub fn new_out(kind: AnimationPresetKind) -> AnimationPreset {
        AnimationPreset::new(kind, Ease::In)
    }

    fn new(kind: AnimationPresetKind, ease: Ease) -> AnimationPreset {
        AnimationPreset {
            kind,
            length: AnimationPreset::LENGTH,
            easing: kind.easing(ease),
            strength: kind.strength(),
            direction: Direction::Left,
            letters: None,
        }
    }

    pub(crate) fn checked(self) -> AnimationPreset {
        AnimationPreset {
            length: self.length.max(0),
            strength: match self.strength.is_finite() {
                true => self.strength,
                false => self.kind.strength(),
            },
            letters: self.letters.map(Letters::checked),
            ..self
        }
    }
}
