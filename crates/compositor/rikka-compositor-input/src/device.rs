use rikka_compositor_geometry::Size;

use crate::space::Millimeters;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(u64);

impl DeviceId {
    pub(crate) const fn new(raw: u64) -> Self {
        Self(raw)
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Capabilities: u8 {
        const KEYBOARD = 1;
        const POINTER = 1 << 1;
        const TOUCH = 1 << 2;
        const TABLET_TOOL = 1 << 3;
        const TABLET_PAD = 1 << 4;
        const GESTURE = 1 << 5;
        const SWITCH = 1 << 6;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointerKind {
    Mouse,
    Touchpad,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceInfo {
    pub id: DeviceId,
    pub name: String,
    pub sysname: String,
    pub vendor: u32,
    pub product: u32,
    pub capabilities: Capabilities,
    pub pointer: Option<PointerKind>,
    pub size: Option<Size<f64, Millimeters>>,
    pub output: Option<String>,
    pub touch_slots: Option<u32>,
    pub pad: Option<PadLayout>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PadLayout {
    pub buttons: u32,
    pub rings: u32,
    pub strips: u32,
    pub groups: Vec<PadGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PadGroup {
    pub index: u32,
    pub modes: u32,
    pub mode: u32,
    pub buttons: Vec<u32>,
    pub toggles: Vec<u32>,
    pub rings: Vec<u32>,
    pub strips: Vec<u32>,
}
