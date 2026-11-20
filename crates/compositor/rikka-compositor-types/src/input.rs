use std::time::Duration;

use rikka_compositor_geometry::{Logical, Point};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Press {
    Pressed,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AxisSource {
    Wheel,
    Finger,
    Continuous,
    WheelTilt,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Gesture {
    SwipeBegin {
        fingers: u32,
        time: Duration,
    },
    SwipeUpdate {
        delta: Point<f64, Logical>,
        time: Duration,
    },
    SwipeEnd {
        cancelled: bool,
        time: Duration,
    },
    PinchBegin {
        fingers: u32,
        time: Duration,
    },
    PinchUpdate {
        delta: Point<f64, Logical>,
        scale: f64,
        rotation: f64,
        time: Duration,
    },
    PinchEnd {
        cancelled: bool,
        time: Duration,
    },
    HoldBegin {
        fingers: u32,
        time: Duration,
    },
    HoldEnd {
        cancelled: bool,
        time: Duration,
    },
}
