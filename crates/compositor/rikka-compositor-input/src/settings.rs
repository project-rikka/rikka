#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Acceleration {
    Adaptive,
    Flat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseScroll {
    Wheel,
    Button,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TouchpadScroll {
    TwoFinger,
    Edge,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TouchpadUse {
    Always,
    Never,
    NoMouse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TapButtons {
    LeftRightMiddle,
    LeftMiddleRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClickMethod {
    Areas,
    Fingers,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MouseSettings {
    pub enabled: bool,
    pub speed: f64,
    pub acceleration: Acceleration,
    pub natural_scroll: bool,
    pub left_handed: bool,
    pub middle_emulation: bool,
    pub scroll: MouseScroll,
    pub scroll_button: u32,
    pub scroll_button_lock: bool,
    pub rotation: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchpadSettings {
    pub used: TouchpadUse,
    pub speed: f64,
    pub acceleration: Acceleration,
    pub natural_scroll: bool,
    pub left_handed: bool,
    pub tap: bool,
    pub tap_drag: bool,
    pub tap_drag_lock: bool,
    pub tap_buttons: TapButtons,
    pub click: ClickMethod,
    pub scroll: TouchpadScroll,
    pub disable_while_typing: bool,
    pub disable_while_pointing: bool,
    pub middle_emulation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeviceSettings {
    pub mouse: MouseSettings,
    pub touchpad: TouchpadSettings,
}
