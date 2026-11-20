use crate::device::DeviceId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Setting {
    Enabled,
    Speed,
    Acceleration,
    NaturalScroll,
    LeftHanded,
    MiddleEmulation,
    ScrollMethod,
    ScrollButton,
    ScrollButtonLock,
    Rotation,
    Tap,
    TapDrag,
    TapDragLock,
    TapButtons,
    ClickMethod,
    DisableWhileTyping,
    DisableWhilePointing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingError {
    Unsupported,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigReport {
    pub device: DeviceId,
    pub rejected: Vec<(Setting, SettingError)>,
}
