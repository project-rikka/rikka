use std::time::Duration;

use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_types::ToolAxes;

use crate::focus::Focus;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bus {
    Usb,
    Bluetooth,
    Virtual,
    Serial,
    I2c,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TabletInfo {
    pub name: String,
    pub vendor: u32,
    pub product: u32,
    pub path: Option<String>,
    pub bus: Option<Bus>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolMotion {
    pub location: Point<f64, Logical>,
    pub under: Option<Focus>,
    pub axes: ToolAxes,
    pub time: Duration,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PadGroupInfo {
    pub buttons: Vec<u32>,
    pub rings: Vec<u32>,
    pub strips: Vec<u32>,
    pub dials: Vec<u32>,
    pub modes: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PadInfo {
    pub path: Option<String>,
    pub buttons: u32,
    pub groups: Vec<PadGroupInfo>,
}
