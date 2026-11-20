use std::time::Duration;

use rikka_ui::{Curve, DurationToken, Motion, Property, motion};
use shell_wallpaper::{CROSSING, CROSSING_CURVE};

const ARRIVING: DurationToken = DurationToken::Medium2;

const RETIRING: DurationToken = DurationToken::Short4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum TransitionStyle {
    #[default]
    Still,
    Edit,
    Crossfade,
}

impl TransitionStyle {
    pub(super) fn arrives(self) -> bool {
        self != TransitionStyle::Still
    }

    pub(super) fn glide(self) -> Curve {
        match self {
            TransitionStyle::Still | TransitionStyle::Edit => motion::GLIDE,
            TransitionStyle::Crossfade => CROSSING_CURVE,
        }
    }

    pub(super) fn arrive(self) -> Curve {
        match self {
            TransitionStyle::Still | TransitionStyle::Edit => {
                Motion::enter(ARRIVING).curve(Property::Spatial)
            }
            TransitionStyle::Crossfade => CROSSING_CURVE,
        }
    }

    pub(super) fn leave(self) -> Curve {
        match self {
            TransitionStyle::Still | TransitionStyle::Edit => {
                Motion::exit(RETIRING).curve(Property::Spatial)
            }
            TransitionStyle::Crossfade => CROSSING_CURVE,
        }
    }

    pub(super) fn leaving_for(self) -> Duration {
        match self {
            TransitionStyle::Still | TransitionStyle::Edit => RETIRING.duration(),
            TransitionStyle::Crossfade => CROSSING.duration(),
        }
    }
}
