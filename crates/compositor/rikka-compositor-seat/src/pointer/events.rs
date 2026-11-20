use std::time::Duration;

use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_types::{AxisSource, Press};

use crate::focus::Focus;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motion {
    pub location: Point<f64, Logical>,
    pub under: Option<Focus>,
    pub time: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RelativeMotion {
    pub delta: Point<f64, Logical>,
    pub unaccelerated: Point<f64, Logical>,
    pub time: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Button {
    pub code: u32,
    pub press: Press,
    pub time: Duration,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AxisMotion {
    pub value: f64,
    pub v120: Option<i32>,
    pub stopped: bool,
    pub inverted: bool,
}

impl AxisMotion {
    pub(crate) fn moved(&self) -> bool {
        self.value != 0.0 || self.v120.is_some_and(|v120| v120 != 0)
    }

    pub(crate) fn is_empty(&self) -> bool {
        !self.moved() && !self.stopped
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Axis {
    pub time: Duration,
    pub source: Option<AxisSource>,
    pub horizontal: AxisMotion,
    pub vertical: AxisMotion,
}

impl Axis {
    pub fn new(time: Duration) -> Self {
        Self {
            time,
            source: None,
            horizontal: AxisMotion::default(),
            vertical: AxisMotion::default(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.horizontal.is_empty() && self.vertical.is_empty()
    }
}
