use rikka_keyframe::{Ease, Easing};

use crate::animation_presets::Direction;

use super::tuning::TransitionKnob as Knob;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransitionFamily {
    Dissolve,
    Wipe,
    Motion,
    Camera,
    Stylize,
}

impl TransitionFamily {
    pub const ALL: [TransitionFamily; 5] = [
        TransitionFamily::Dissolve,
        TransitionFamily::Wipe,
        TransitionFamily::Motion,
        TransitionFamily::Camera,
        TransitionFamily::Stylize,
    ];

    pub fn kinds(self) -> impl Iterator<Item = TransitionKind> {
        TransitionKind::ALL
            .into_iter()
            .filter(move |kind| kind.family() == self)
    }

    fn seconds(self) -> f64 {
        match self {
            TransitionFamily::Dissolve | TransitionFamily::Wipe | TransitionFamily::Motion => 0.7,
            TransitionFamily::Camera | TransitionFamily::Stylize => 0.35,
        }
    }

    fn easing(self) -> Easing {
        match self {
            TransitionFamily::Dissolve => Easing::LINEAR,
            TransitionFamily::Wipe => Easing::new(rikka_keyframe::Family::Sine, Ease::InOut),
            TransitionFamily::Motion => Easing::new(rikka_keyframe::Family::Quint, Ease::InOut),
            TransitionFamily::Camera | TransitionFamily::Stylize => {
                Easing::new(rikka_keyframe::Family::Quart, Ease::InOut)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransitionKind {
    CrossDissolve,
    FilmDissolve,
    AdditiveDissolve,
    Dip,
    BlurDissolve,
    NoiseDissolve,
    Wipe,
    Split,
    IrisRound,
    IrisBox,
    Push,
    SlideIn,
    SlideOut,
    CrossZoom,
    Turn,
    ZoomIn,
    ZoomOut,
    Spin,
    WhipPan,
    Stretch,
    Shake,
    Glitch,
    ChromaSplit,
    Flash,
    LightLeak,
    FilmBurn,
    Pixelate,
    LensWarp,
    LiquidWarp,
}

use TransitionFamily as Family;
use TransitionKind as Kind;

const WIPES: [Direction; 8] = [
    Direction::Right,
    Direction::Left,
    Direction::Down,
    Direction::Up,
    Direction::DownRight,
    Direction::DownLeft,
    Direction::UpRight,
    Direction::UpLeft,
];
const FOUR: [Direction; 4] = [
    Direction::Left,
    Direction::Right,
    Direction::Up,
    Direction::Down,
];
const AXES: [Direction; 2] = [Direction::Left, Direction::Up];
const TURNS: [Direction; 2] = [Direction::Right, Direction::Left];

impl TransitionKind {
    pub const ALL: [TransitionKind; 29] = [
        Kind::CrossDissolve,
        Kind::FilmDissolve,
        Kind::AdditiveDissolve,
        Kind::Dip,
        Kind::BlurDissolve,
        Kind::NoiseDissolve,
        Kind::Wipe,
        Kind::Split,
        Kind::IrisRound,
        Kind::IrisBox,
        Kind::Push,
        Kind::SlideIn,
        Kind::SlideOut,
        Kind::CrossZoom,
        Kind::Turn,
        Kind::ZoomIn,
        Kind::ZoomOut,
        Kind::Spin,
        Kind::WhipPan,
        Kind::Stretch,
        Kind::Shake,
        Kind::Glitch,
        Kind::ChromaSplit,
        Kind::Flash,
        Kind::LightLeak,
        Kind::FilmBurn,
        Kind::Pixelate,
        Kind::LensWarp,
        Kind::LiquidWarp,
    ];

    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    #[must_use]
    pub fn from_index(index: usize) -> Option<TransitionKind> {
        TransitionKind::ALL.get(index).copied()
    }
    #[must_use]
    pub fn family(self) -> TransitionFamily {
        match self {
            Kind::CrossDissolve
            | Kind::FilmDissolve
            | Kind::AdditiveDissolve
            | Kind::Dip
            | Kind::BlurDissolve
            | Kind::NoiseDissolve => Family::Dissolve,
            Kind::Wipe | Kind::Split | Kind::IrisRound | Kind::IrisBox => Family::Wipe,
            Kind::Push | Kind::SlideIn | Kind::SlideOut | Kind::CrossZoom | Kind::Turn => {
                Family::Motion
            }
            Kind::ZoomIn
            | Kind::ZoomOut
            | Kind::Spin
            | Kind::WhipPan
            | Kind::Stretch
            | Kind::Shake
            | Kind::LensWarp => Family::Camera,
            Kind::Glitch
            | Kind::ChromaSplit
            | Kind::Flash
            | Kind::LightLeak
            | Kind::FilmBurn
            | Kind::Pixelate
            | Kind::LiquidWarp => Family::Stylize,
        }
    }

    #[must_use]
    pub fn knobs(self) -> &'static [Knob] {
        match self {
            Kind::CrossDissolve | Kind::FilmDissolve | Kind::AdditiveDissolve | Kind::Turn => &[],
            Kind::Dip => &[Knob::Tint],
            Kind::BlurDissolve => &[Knob::Blur],
            Kind::NoiseDissolve => &[Knob::Softness, Knob::Size],
            Kind::Wipe | Kind::Split | Kind::IrisRound | Kind::IrisBox => &[Knob::Softness],
            Kind::Push | Kind::SlideIn | Kind::SlideOut | Kind::WhipPan => &[Knob::Shutter],
            Kind::ZoomIn
            | Kind::ZoomOut
            | Kind::Spin
            | Kind::Stretch
            | Kind::Shake
            | Kind::LensWarp => &[Knob::Amount, Knob::Shutter],
            Kind::CrossZoom | Kind::Glitch | Kind::ChromaSplit | Kind::Pixelate => &[Knob::Amount],
            Kind::Flash | Kind::LightLeak | Kind::FilmBurn => &[Knob::Amount, Knob::Tint],
            Kind::LiquidWarp => &[Knob::Amount, Knob::Size],
        }
    }

    #[must_use]
    pub fn has_knob(self, knob: Knob) -> bool {
        self.knobs().contains(&knob)
    }

    #[must_use]
    pub fn directions(self) -> &'static [Direction] {
        match self {
            Kind::Wipe => &WIPES,
            Kind::Push | Kind::SlideIn | Kind::SlideOut | Kind::Turn | Kind::WhipPan => &FOUR,
            Kind::Split | Kind::Stretch => &AXES,
            Kind::Spin => &TURNS,
            _ => &[],
        }
    }

    #[must_use]
    pub fn direction(self) -> Direction {
        self.directions()
            .first()
            .copied()
            .unwrap_or(Direction::Left)
    }

    #[must_use]
    pub fn seconds(self) -> f64 {
        self.family().seconds()
    }

    #[must_use]
    pub fn easing(self) -> Easing {
        self.family().easing()
    }
}
