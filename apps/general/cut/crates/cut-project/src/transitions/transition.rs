use rikka_keyframe::Easing;

use crate::animation_presets::Direction;
use crate::rate::Rate;

use super::kind::TransitionKind as Kind;
use super::tuning::Tuning;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransitionAlignment {
    Centre,
    Start,
    End,
}

impl TransitionAlignment {
    pub const ALL: [TransitionAlignment; 3] = [
        TransitionAlignment::End,
        TransitionAlignment::Centre,
        TransitionAlignment::Start,
    ];

    fn split(self, length: i64) -> (i64, i64) {
        match self {
            TransitionAlignment::Centre => (length / 2, length - length / 2),
            TransitionAlignment::Start => (0, length),
            TransitionAlignment::End => (length, 0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    pub kind: Kind,
    pub before: i64,
    pub after: i64,
    pub easing: Easing,
    pub direction: Direction,
    pub tuning: Tuning,
    pub sound: bool,
}

impl Transition {
    pub const LONGEST: i64 = 100_000;

    #[must_use]
    pub fn new(kind: Kind, rate: Rate) -> Transition {
        let length = (kind.seconds() * rate.per_second()).round().max(2.0) as i64;
        let (before, after) = TransitionAlignment::Centre.split(length);
        Transition {
            kind,
            before,
            after,
            easing: kind.easing(),
            direction: kind.direction(),
            tuning: Tuning::default_for(kind),
            sound: true,
        }
    }

    #[must_use]
    pub fn length(&self) -> i64 {
        self.before + self.after
    }

    #[must_use]
    pub fn alignment_at_cut(&self) -> Option<TransitionAlignment> {
        TransitionAlignment::ALL
            .into_iter()
            .find(|at| at.split(self.length()) == (self.before, self.after))
    }

    #[must_use]
    pub fn with_alignment(self, at: TransitionAlignment) -> Transition {
        let (before, after) = at.split(self.length());
        Transition {
            before,
            after,
            ..self
        }
    }

    #[must_use]
    pub fn with_length(self, length: i64) -> Transition {
        let length = length.clamp(1, Transition::LONGEST);
        let (before, after) = match self.alignment_at_cut() {
            Some(at) => at.split(length),
            None => {
                let share = self.before as f64 / self.length().max(1) as f64;
                let before = (share * length as f64).round() as i64;
                (before, length - before)
            }
        };
        Transition {
            before,
            after,
            ..self
        }
    }

    #[must_use]
    pub fn with_kind(self, kind: Kind) -> Transition {
        if kind == self.kind {
            return self;
        }
        Transition {
            kind,
            easing: kind.easing(),
            direction: kind.direction(),
            tuning: Tuning::default_for(kind),
            ..self
        }
    }

    pub(crate) fn checked(self) -> Transition {
        let before = self.before.clamp(0, Transition::LONGEST);
        let after = self.after.clamp(0, Transition::LONGEST);
        let (before, after) = match before + after {
            0 => (0, 1),
            _ => (before, after),
        };
        Transition {
            before,
            after,
            direction: match self.kind.directions().contains(&self.direction) {
                true => self.direction,
                false => self.kind.direction(),
            },
            tuning: self.tuning.checked(self.kind),
            ..self
        }
    }
}
