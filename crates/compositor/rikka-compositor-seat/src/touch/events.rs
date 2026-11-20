use std::time::Duration;

use rikka_compositor_geometry::{Logical, Point};

use crate::focus::Focus;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchDown {
    pub slot: u32,
    pub location: Point<f64, Logical>,
    pub under: Option<Focus>,
    pub time: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchMotion {
    pub slot: u32,
    pub location: Point<f64, Logical>,
    pub under: Option<Focus>,
    pub time: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TouchUp {
    pub slot: u32,
    pub time: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchShape {
    pub slot: u32,
    pub major: f64,
    pub minor: f64,
    pub orientation: Option<f64>,
}
