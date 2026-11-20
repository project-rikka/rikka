use rikka_types::Point;

use crate::affine::Affine;
use crate::form::FormKind;
use crate::frame::Frame;
use crate::grips::{Grip, Grips};

const TURN_STEP: f32 = 15.0;
const AIM_STEP: f32 = 15.0;
const MIN_EXTENT: f32 = 1e-3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Keys {
    pub shift: bool,
    pub alt: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrabTarget {
    Body,
    Grip(Grip),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Move,
    Resize,
    Rotate,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ManipulationResult {
    Mapped { map: Affine, grips: Grips },
    Ended { end: usize, from: Point, to: Point },
}

impl ManipulationResult {
    #[must_use]
    pub fn grips(&self) -> Grips {
        match *self {
            ManipulationResult::Mapped { grips, .. } => grips,
            ManipulationResult::Ended { from, to, .. } => Grips::Ends(from, to),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Manipulation {
    grabbed: GrabTarget,
    from: Point,
    grips: Grips,
    whole: bool,
}

impl Manipulation {
    #[must_use]
    pub fn new(grabbed: GrabTarget, from: Point, grips: Grips) -> Manipulation {
        Manipulation {
            grabbed,
            from,
            grips,
            whole: true,
        }
    }

    #[must_use]
    pub fn fractional(self) -> Manipulation {
        Manipulation {
            whole: false,
            ..self
        }
    }

    #[must_use]
    pub fn target(&self) -> GrabTarget {
        self.grabbed
    }

    #[must_use]
    pub fn grips(&self) -> Grips {
        self.grips
    }

    #[must_use]
    pub fn change(&self) -> Change {
        match self.grabbed {
            GrabTarget::Body => Change::Move,
            GrabTarget::Grip(Grip::Turn) => Change::Rotate,
            GrabTarget::Grip(Grip::End(_) | Grip::Side(..)) => Change::Resize,
        }
    }

    #[must_use]
    pub fn drag_to(&self, at: Point, keys: Keys) -> ManipulationResult {
        let unchanged = ManipulationResult::Mapped {
            map: Affine::IDENTITY,
            grips: self.grips,
        };
        match (self.grabbed, self.grips) {
            (GrabTarget::Body, _) => self.mapped(self.moved(at - self.from, keys)),
            (GrabTarget::Grip(Grip::End(end)), Grips::Ends(from, to)) => {
                let (from, to) = match end {
                    0 => (constrain_angle(to, at, keys), to),
                    _ => (from, constrain_angle(from, at, keys)),
                };
                ManipulationResult::Ended { end, from, to }
            }
            (GrabTarget::Grip(Grip::Side(x, y)), Grips::Frame { frame, sides, .. }) => {
                let uniform = keys.shift || !sides;
                self.mapped(sized(&frame, (x, y), at, uniform, keys.alt))
            }
            (GrabTarget::Grip(Grip::Turn), Grips::Frame { frame, .. }) => {
                let turn = |point: Point| {
                    let offset = point - frame.center;
                    offset.y.atan2(offset.x).to_degrees()
                };
                let mut degrees = turn(at) - turn(self.from);
                if keys.shift {
                    let aimed = ((frame.angle + degrees) / TURN_STEP).round() * TURN_STEP;
                    degrees = aimed - frame.angle;
                }
                self.mapped(Affine::about(
                    frame.center,
                    Affine::rotation(f64::from(degrees)),
                ))
            }
            (GrabTarget::Grip(_), _) => unchanged,
        }
    }

    fn mapped(&self, map: Affine) -> ManipulationResult {
        ManipulationResult::Mapped {
            map,
            grips: self.grips.mapped(map),
        }
    }

    fn moved(&self, by: Point, keys: Keys) -> Affine {
        let by = match keys.shift {
            true => {
                let (across, down) = (by.x.abs(), by.y.abs());
                if across > down * 2.0 {
                    Point::new(by.x, 0.0)
                } else if down > across * 2.0 {
                    Point::new(0.0, by.y)
                } else {
                    let side = (across + down) / 2.0;
                    Point::new(side.copysign(by.x), side.copysign(by.y))
                }
            }
            false => by,
        };
        let by = match self.whole {
            true => Point::new(by.x.round(), by.y.round()),
            false => by,
        };
        Affine::translation(f64::from(by.x), f64::from(by.y))
    }
}

#[must_use]
pub fn constrain_angle(anchor: Point, at: Point, keys: Keys) -> Point {
    if !keys.shift {
        return at;
    }
    let (across, down) = (at.x - anchor.x, at.y - anchor.y);
    let length = across.hypot(down);
    if length <= f32::EPSILON {
        return at;
    }
    let step = AIM_STEP.to_radians();
    let angle = (down.atan2(across) / step).round() * step;
    Point::new(
        anchor.x + length * angle.cos(),
        anchor.y + length * angle.sin(),
    )
}

#[must_use]
pub fn constrained_span(kind: FormKind, anchor: Point, at: Point, keys: Keys) -> (Point, Point) {
    match kind {
        FormKind::Line | FormKind::Arrow => (anchor, constrain_angle(anchor, at, keys)),
        FormKind::Shape(_) => {
            let to = match keys.shift {
                true => {
                    let side = (at.x - anchor.x).abs().max((at.y - anchor.y).abs());
                    Point::new(
                        anchor.x + side.copysign(at.x - anchor.x),
                        anchor.y + side.copysign(at.y - anchor.y),
                    )
                }
                false => at,
            };
            match keys.alt {
                true => (anchor * 2.0 - to, to),
                false => (anchor, to),
            }
        }
        FormKind::Path | FormKind::Text => (anchor, at),
    }
}

fn sized(frame: &Frame, (x, y): (i8, i8), at: Point, uniform: bool, centered: bool) -> Affine {
    let local = frame.to_local(at);
    let (half_width, half_height) = (frame.width / 2.0, frame.height / 2.0);
    let anchor = match centered {
        true => Point::ZERO,
        false => Point::new(-f32::from(x) * half_width, -f32::from(y) * half_height),
    };
    let scale = |side: i8, half: f32, anchor: f32, reached: f32| {
        let reach = f32::from(side) * half - anchor;
        match side != 0 && reach.abs() > MIN_EXTENT {
            true => (reached - anchor) / reach,
            false => 1.0,
        }
    };
    let mut across = scale(x, half_width, anchor.x, local.x);
    let mut down = scale(y, half_height, anchor.y, local.y);
    if uniform {
        let both = match (x, y) {
            (0, _) => down.abs(),
            (_, 0) => across.abs(),
            _ => across.abs().max(down.abs()),
        };
        across = both.copysign(across);
        down = both.copysign(down);
    }
    let center = frame.center;
    let angle = f64::from(frame.angle);
    Affine::translation(-f64::from(center.x), -f64::from(center.y))
        .then(Affine::rotation(-angle))
        .then(Affine::translation(
            -f64::from(anchor.x),
            -f64::from(anchor.y),
        ))
        .then(Affine::scale(f64::from(across), f64::from(down)))
        .then(Affine::translation(
            f64::from(anchor.x),
            f64::from(anchor.y),
        ))
        .then(Affine::rotation(angle))
        .then(Affine::translation(
            f64::from(center.x),
            f64::from(center.y),
        ))
}
