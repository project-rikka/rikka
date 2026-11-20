use std::ops::Add;

use rikka_animation::{Curve, DurationToken, Easing, Spring};
use rikka_types::{Point, Size};

use crate::motion::{self, Wipe};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Pose {
    pub(crate) opacity: f32,
    pub(crate) scale: f32,
    pub(crate) shift: Point,
    pub(crate) nudge: Point,
    pub(crate) wipe: Option<Wipe>,
    pub(crate) about_anchor: bool,
}

impl Pose {
    pub(crate) const REST: Self = Self {
        opacity: 1.0,
        scale: 1.0,
        shift: Point::ZERO,
        nudge: Point::ZERO,
        wipe: None,
        about_anchor: false,
    };

    pub(crate) fn toward_rest(self, progress: f32, size: Size) -> Pose {
        self.blend(Pose::REST, progress, size)
    }

    pub(crate) fn blend(self, toward: Pose, amount: f32, size: Size) -> Pose {
        let away = 1.0 - amount;
        let mix = |from: f32, to: f32| to + (from - to) * away;
        let (from, to) = (self.travel(size), toward.travel(size));
        Pose {
            opacity: mix(self.opacity, toward.opacity),
            scale: mix(self.scale, toward.scale),
            shift: Point::ZERO,
            nudge: Point::new(mix(from.x, to.x), mix(from.y, to.y)),
            wipe: self.wipe.or(toward.wipe),
            about_anchor: self.about_anchor || toward.about_anchor,
        }
    }

    fn travel(self, size: Size) -> Point {
        Point::new(
            self.shift.x * size.width + self.nudge.x,
            self.shift.y * size.height + self.nudge.y,
        )
    }
}

impl Add for Pose {
    type Output = Pose;

    fn add(self, other: Pose) -> Pose {
        Pose {
            opacity: self.opacity * other.opacity,
            scale: self.scale * other.scale,
            shift: self.shift + other.shift,
            nudge: self.nudge + other.nudge,
            wipe: self.wipe.or(other.wipe),
            about_anchor: self.about_anchor || other.about_anchor,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Top,
    Bottom,
    Leading,
    Trailing,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    pub(crate) arrive: Pose,
    pub(crate) depart: Pose,
    pub(crate) enter: Curve,
    pub(crate) exit: Curve,
    pub(crate) delay: Option<DurationToken>,
}

impl Transition {
    #[must_use]
    pub const fn enter(mut self, curve: Curve) -> Self {
        self.enter = curve;
        self
    }

    #[must_use]
    pub const fn exit(mut self, curve: Curve) -> Self {
        self.exit = curve;
        self
    }

    #[must_use]
    pub const fn delay(mut self, delay: DurationToken) -> Self {
        self.delay = Some(delay);
        self
    }
}

impl Add for Transition {
    type Output = Transition;

    fn add(self, other: Transition) -> Transition {
        Transition {
            arrive: self.arrive + other.arrive,
            depart: self.depart + other.depart,
            enter: self.enter,
            exit: self.exit,
            delay: self.delay.max(other.delay),
        }
    }
}

const ENTER: Curve = Curve::Tween {
    easing: Easing::EmphasizedDecel,
    duration: DurationToken::Medium2,
};

const EXIT: Curve = Curve::Tween {
    easing: Easing::EmphasizedAccel,
    duration: DurationToken::Short3,
};

const AXIS_TRAVEL: f32 = 30.0;

const fn symmetric(pose: Pose, enter: Curve, exit: Curve) -> Transition {
    Transition {
        arrive: pose,
        depart: pose,
        enter,
        exit,
        delay: None,
    }
}

const fn faded(opacity: f32, scale: f32) -> Pose {
    Pose {
        opacity,
        scale,
        shift: Point::ZERO,
        nudge: Point::ZERO,
        wipe: None,
        about_anchor: false,
    }
}

pub const fn fade() -> Transition {
    symmetric(
        faded(0.0, 1.0),
        Curve::Tween {
            easing: Easing::Standard,
            duration: DurationToken::Short3,
        },
        Curve::Tween {
            easing: Easing::StandardAccel,
            duration: DurationToken::Short2,
        },
    )
}

pub const fn scale(from: f32) -> Transition {
    symmetric(faded(1.0, from), ENTER, EXIT)
}

pub const fn slide(edge: Edge) -> Transition {
    let shift = match edge {
        Edge::Top => Point::new(0.0, -1.0),
        Edge::Bottom => Point::new(0.0, 1.0),
        Edge::Leading => Point::new(-1.0, 0.0),
        Edge::Trailing => Point::new(1.0, 0.0),
    };
    symmetric(
        Pose {
            shift,
            ..Pose::REST
        },
        motion::SLIDE_IN,
        motion::SLIDE_OUT,
    )
}

pub const fn wipe(direction: Wipe) -> Transition {
    symmetric(
        Pose {
            wipe: Some(direction),
            ..Pose::REST
        },
        Curve::Spring(Spring::DEFAULT_SPATIAL),
        EXIT,
    )
}

pub const fn offset(x: f32, y: f32) -> Transition {
    symmetric(
        Pose {
            nudge: Point::new(x, y),
            ..Pose::REST
        },
        ENTER,
        EXIT,
    )
}

pub const fn container_transform() -> Transition {
    symmetric(
        faded(0.0, 0.85),
        Curve::Spring(Spring::DEFAULT_SPATIAL),
        EXIT,
    )
}

pub const fn fade_scale() -> Transition {
    Transition {
        arrive: faded(0.0, 0.8),
        depart: faded(0.0, 1.0),
        enter: Curve::Tween {
            easing: Easing::EmphasizedDecel,
            duration: DurationToken::Short3,
        },
        exit: Curve::Tween {
            easing: Easing::Linear,
            duration: DurationToken::Short2,
        },
        delay: None,
    }
}

pub(crate) const POPPED: Pose = Pose {
    opacity: 0.0,
    scale: 0.92,
    about_anchor: true,
    ..Pose::REST
};

pub const fn pop() -> Transition {
    symmetric(POPPED, motion::POP, motion::POP_OUT)
}

pub const fn fade_through() -> Transition {
    Transition {
        arrive: faded(0.0, 0.92),
        depart: faded(0.0, 1.0),
        enter: Curve::Tween {
            easing: Easing::Emphasized,
            duration: DurationToken::Medium1,
        },
        exit: Curve::Tween {
            easing: Easing::StandardAccel,
            duration: DurationToken::Short2,
        },
        delay: Some(DurationToken::Short2),
    }
}

const fn axis(arrive: Point, depart: Point) -> Transition {
    Transition {
        arrive: Pose {
            opacity: 0.0,
            nudge: arrive,
            ..Pose::REST
        },
        depart: Pose {
            opacity: 0.0,
            nudge: depart,
            ..Pose::REST
        },
        enter: Curve::Tween {
            easing: Easing::Emphasized,
            duration: DurationToken::Medium2,
        },
        exit: Curve::Tween {
            easing: Easing::EmphasizedAccel,
            duration: DurationToken::Short3,
        },
        delay: None,
    }
}

pub const fn shared_axis_x() -> Transition {
    axis(Point::new(AXIS_TRAVEL, 0.0), Point::new(-AXIS_TRAVEL, 0.0))
}

pub const fn shared_axis_y() -> Transition {
    axis(Point::new(0.0, AXIS_TRAVEL), Point::new(0.0, -AXIS_TRAVEL))
}

pub const fn shared_axis_z() -> Transition {
    Transition {
        arrive: faded(0.0, 0.8),
        depart: faded(0.0, 1.1),
        enter: Curve::Tween {
            easing: Easing::Emphasized,
            duration: DurationToken::Medium2,
        },
        exit: Curve::Tween {
            easing: Easing::EmphasizedAccel,
            duration: DurationToken::Short3,
        },
        delay: None,
    }
}
