use rikka_keyframe::{Easing, Framed};
use rikka_types::Point;

use super::kind::Kind;

const THIRD: f32 = 1.0 / 3.0;
const SAME: f32 = 1e-4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Handle {
    pub speed: f32,
    pub influence: f32,
}

impl Handle {
    pub const MIN_INFLUENCE: f32 = 0.001;
    pub const EASED: Handle = Handle {
        speed: 0.0,
        influence: THIRD,
    };

    #[must_use]
    pub fn new(speed: f32, influence: f32) -> Handle {
        Handle { speed, influence }
    }

    #[must_use]
    pub fn checked(self) -> Handle {
        Handle {
            speed: match self.speed.is_finite() {
                true => self.speed,
                false => 0.0,
            },
            influence: match self.influence.is_finite() {
                true => self.influence.clamp(Handle::MIN_INFLUENCE, 1.0),
                false => THIRD,
            },
        }
    }

    fn is_eased(self) -> bool {
        self.speed == 0.0 && (self.influence - THIRD).abs() < SAME
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Incoming {
    #[default]
    Linear,
    Bezier(Handle),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preset {
    pub easing: Easing,
    pub from: f32,
    pub to: f32,
}

impl Preset {
    #[must_use]
    pub fn whole(easing: Easing) -> Preset {
        Preset {
            easing,
            from: 0.0,
            to: 1.0,
        }
    }

    #[must_use]
    pub fn is_whole(&self) -> bool {
        self.from == 0.0 && self.to == 1.0
    }

    pub(crate) fn reversed(self) -> Preset {
        Preset {
            easing: self.easing.reversed(),
            from: 1.0 - self.to,
            to: 1.0 - self.from,
        }
    }

    pub(crate) fn is_valid(&self) -> bool {
        self.from.is_finite()
            && self.to.is_finite()
            && 0.0 <= self.from
            && self.from < self.to
            && self.to <= 1.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Outgoing {
    Hold,
    #[default]
    Linear,
    Bezier(Handle),
    Preset(Preset),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Smooth {
    #[default]
    Manual,
    Continuous,
    Auto,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Incoming,
    Outgoing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Interpolation {
    Linear,
    Bezier,
    Continuous,
    Auto,
    Hold,
    EasyEase,
    EasyEaseIn,
    EasyEaseOut,
    Preset(Easing),
}

impl Interpolation {
    pub const PLAIN: [Interpolation; 5] = [
        Interpolation::Linear,
        Interpolation::Bezier,
        Interpolation::Continuous,
        Interpolation::Auto,
        Interpolation::Hold,
    ];
    pub const EASY: [Interpolation; 3] = [
        Interpolation::EasyEase,
        Interpolation::EasyEaseIn,
        Interpolation::EasyEaseOut,
    ];
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Timing {
    pub incoming: Incoming,
    pub outgoing: Outgoing,
    pub smooth: Smooth,
}

impl Timing {
    pub const LINEAR: Timing = Timing {
        incoming: Incoming::Linear,
        outgoing: Outgoing::Linear,
        smooth: Smooth::Manual,
    };

    #[must_use]
    pub fn handle(&self, side: Side) -> Option<Handle> {
        match (side, self.incoming, self.outgoing) {
            (Side::Incoming, Incoming::Bezier(handle), _) => Some(handle),
            (Side::Outgoing, _, Outgoing::Bezier(handle)) => Some(handle),
            _ => None,
        }
    }

    #[must_use]
    pub fn interpolation(&self) -> Interpolation {
        let eased_in = matches!(self.incoming, Incoming::Bezier(handle) if handle.is_eased());
        let eased_out = matches!(self.outgoing, Outgoing::Bezier(handle) if handle.is_eased());
        match (self.incoming, self.outgoing, self.smooth) {
            (_, Outgoing::Hold, _) => Interpolation::Hold,
            (_, Outgoing::Preset(preset), _) => Interpolation::Preset(preset.easing),
            (Incoming::Linear, Outgoing::Linear, _) => Interpolation::Linear,
            (_, _, Smooth::Auto) => Interpolation::Auto,
            _ if eased_in && eased_out => Interpolation::EasyEase,
            (_, Outgoing::Linear, _) if eased_in => Interpolation::EasyEaseIn,
            (Incoming::Linear, _, _) if eased_out => Interpolation::EasyEaseOut,
            (_, _, Smooth::Continuous) => Interpolation::Continuous,
            _ => Interpolation::Bezier,
        }
    }

    #[must_use]
    pub fn with_handle(self, side: Side, handle: Handle) -> Timing {
        let handle = handle.checked();
        let smooth = match self.smooth {
            Smooth::Auto => Smooth::Continuous,
            other => other,
        };
        let mirrored = |other: Handle| Handle {
            speed: handle.speed,
            ..other
        };
        match side {
            Side::Incoming => Timing {
                incoming: Incoming::Bezier(handle),
                outgoing: match (smooth, self.outgoing) {
                    (Smooth::Continuous, Outgoing::Bezier(other)) => {
                        Outgoing::Bezier(mirrored(other))
                    }
                    (_, outgoing) => outgoing,
                },
                smooth,
            },
            Side::Outgoing => Timing {
                incoming: match (smooth, self.incoming) {
                    (Smooth::Continuous, Incoming::Bezier(other)) => {
                        Incoming::Bezier(mirrored(other))
                    }
                    (_, incoming) => incoming,
                },
                outgoing: Outgoing::Bezier(handle),
                smooth,
            },
        }
    }

    pub(crate) fn checked(self) -> Option<Timing> {
        let incoming = match (self.smooth, self.incoming, self.outgoing) {
            (Smooth::Continuous, Incoming::Bezier(handle), Outgoing::Bezier(leading)) => {
                Incoming::Bezier(
                    Handle {
                        speed: leading.speed,
                        ..handle
                    }
                    .checked(),
                )
            }
            (_, Incoming::Bezier(handle), _) => Incoming::Bezier(handle.checked()),
            (_, Incoming::Linear, _) => Incoming::Linear,
        };
        let outgoing = match self.outgoing {
            Outgoing::Bezier(handle) => Outgoing::Bezier(handle.checked()),
            Outgoing::Preset(preset) if !preset.is_valid() => return None,
            other => other,
        };
        Some(Timing {
            incoming,
            outgoing,
            smooth: self.smooth,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bend {
    pub incoming: Point,
    pub outgoing: Point,
    pub smooth: Smooth,
    pub straight: bool,
}

impl Default for Bend {
    fn default() -> Bend {
        Bend {
            incoming: Point::ZERO,
            outgoing: Point::ZERO,
            smooth: Smooth::Auto,
            straight: false,
        }
    }
}

impl Bend {
    pub const STRAIGHT: Bend = Bend {
        incoming: Point::ZERO,
        outgoing: Point::ZERO,
        smooth: Smooth::Manual,
        straight: true,
    };

    pub(crate) fn reversed(self) -> Bend {
        Bend {
            incoming: self.outgoing,
            outgoing: self.incoming,
            ..self
        }
    }

    #[must_use]
    pub(crate) fn with_tangent(self, side: Side, tangent: Point) -> Bend {
        let smooth = match self.smooth {
            Smooth::Auto => Smooth::Continuous,
            other => other,
        };
        let opposite = |other: Point| {
            let length = (tangent.x.hypot(tangent.y), other.x.hypot(other.y));
            match length.0 > f32::EPSILON {
                true => tangent * (-length.1 / length.0),
                false => other,
            }
        };
        let (incoming, outgoing) = match (side, smooth) {
            (Side::Incoming, Smooth::Continuous) => (tangent, opposite(self.outgoing)),
            (Side::Outgoing, Smooth::Continuous) => (opposite(self.incoming), tangent),
            (Side::Incoming, _) => (tangent, self.outgoing),
            (Side::Outgoing, _) => (self.incoming, tangent),
        };
        Bend {
            incoming,
            outgoing,
            smooth,
            straight: false,
        }
    }

    #[must_use]
    pub fn tangents(&self) -> (Point, Point) {
        match self.straight {
            true => (Point::ZERO, Point::ZERO),
            false => (self.incoming, self.outgoing),
        }
    }

    pub(crate) fn is_finite(&self) -> bool {
        [self.incoming, self.outgoing]
            .iter()
            .all(|tangent| tangent.x.is_finite() && tangent.y.is_finite())
    }

    pub(crate) fn continued(self) -> Bend {
        let leading = self.outgoing.x.hypot(self.outgoing.y);
        match self.smooth == Smooth::Continuous && leading > f32::EPSILON {
            true => Bend {
                incoming: self.outgoing * (-self.incoming.x.hypot(self.incoming.y) / leading),
                ..self
            },
            false => self,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key<K: Kind> {
    pub at: i64,
    pub value: K::Value,
    pub timing: Timing,
    pub bend: K::Bend,
}

impl<K: Kind> Framed for Key<K> {
    type Frame = i64;

    fn frame(&self) -> i64 {
        self.at
    }
}

impl<K: Kind> Key<K> {
    #[must_use]
    pub fn new(at: i64, value: K::Value) -> Key<K> {
        Key {
            at,
            value,
            timing: Timing::LINEAR,
            bend: K::Bend::default(),
        }
    }

    #[must_use]
    pub fn with_timing(self, timing: Timing) -> Key<K> {
        Key { timing, ..self }
    }

    #[must_use]
    pub fn with_outgoing(self, outgoing: Outgoing) -> Key<K> {
        Key {
            timing: Timing {
                outgoing,
                ..self.timing
            },
            ..self
        }
    }

    #[must_use]
    pub fn with_incoming(self, incoming: Incoming) -> Key<K> {
        Key {
            timing: Timing {
                incoming,
                ..self.timing
            },
            ..self
        }
    }

    #[must_use]
    pub fn with_bend(self, bend: K::Bend) -> Key<K> {
        Key { bend, ..self }
    }
}
