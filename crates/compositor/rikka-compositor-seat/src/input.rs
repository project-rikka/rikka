use std::time::Duration;

use rikka_compositor_types::Press;

use crate::ids::VirtualKeyboardId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InputDevice(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeySource {
    Device(InputDevice),
    Virtual(VirtualKeyboardId),
    InputMethod(VirtualKeyboardId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    pub code: u32,
    pub press: Press,
    pub time: Duration,
    pub source: KeySource,
}

pub(crate) fn millis(time: Duration) -> u32 {
    time.as_millis() as u32
}
