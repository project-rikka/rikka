use rikka_compositor_input::{DeviceSettings, MouseSettings, TouchpadSettings};

macro_rules! named {
    (
        $(#[$meta:meta])*
        enum $name:ident { $($variant:ident = $written:literal),+ $(,)? }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum $name { $($variant),+ }

        impl $name {
            pub(crate) fn from_name(name: &str) -> Option<Self> {
                match name {
                    $($written => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

named! {
    enum Acceleration {
        Adaptive = "adaptive",
        Flat = "flat",
    }
}

named! {
    enum MouseScroll {
        Wheel = "wheel",
        Button = "button",
    }
}

named! {
    enum TouchpadScroll {
        TwoFinger = "two-finger",
        Edge = "edge",
        None = "none",
    }
}

named! {
    enum TouchpadUse {
        Always = "always",
        Never = "never",
        NoMouse = "no-mouse",
    }
}

named! {
    enum TapButtons {
        LeftRightMiddle = "left-right-middle",
        LeftMiddleRight = "left-middle-right",
    }
}

named! {
    enum ClickMethod {
        Areas = "areas",
        Fingers = "fingers",
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MouseRules {
    pub(crate) enabled: bool,
    pub(crate) speed: f64,
    pub(crate) acceleration: Acceleration,
    pub(crate) natural_scroll: bool,
    pub(crate) left_handed: bool,
    pub(crate) middle_emulation: bool,
    pub(crate) scroll: MouseScroll,
    pub(crate) scroll_button: u32,
    pub(crate) scroll_button_lock: bool,
    pub(crate) scroll_factor: f64,
    pub(crate) rotation: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TouchpadRules {
    pub(crate) used: TouchpadUse,
    pub(crate) speed: f64,
    pub(crate) acceleration: Acceleration,
    pub(crate) natural_scroll: bool,
    pub(crate) left_handed: bool,
    pub(crate) tap: bool,
    pub(crate) tap_drag: bool,
    pub(crate) tap_drag_lock: bool,
    pub(crate) tap_buttons: TapButtons,
    pub(crate) click: ClickMethod,
    pub(crate) scroll: TouchpadScroll,
    pub(crate) disable_while_typing: bool,
    pub(crate) disable_while_pointing: bool,
    pub(crate) middle_emulation: bool,
    pub(crate) scroll_factor: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CursorRules {
    pub(crate) theme: Option<String>,
    pub(crate) size: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PointerRules {
    pub(crate) mouse: MouseRules,
    pub(crate) touchpad: TouchpadRules,
    pub(crate) cursor: CursorRules,
}

impl PointerRules {
    pub(crate) fn declared() -> Self {
        Self {
            mouse: MouseRules {
                enabled: true,
                speed: 0.0,
                acceleration: Acceleration::Adaptive,
                natural_scroll: false,
                left_handed: false,
                middle_emulation: false,
                scroll: MouseScroll::Wheel,
                scroll_button: 274,
                scroll_button_lock: false,
                scroll_factor: 1.0,
                rotation: 0,
            },
            touchpad: TouchpadRules {
                used: TouchpadUse::Always,
                speed: 0.0,
                acceleration: Acceleration::Adaptive,
                natural_scroll: true,
                left_handed: false,
                tap: true,
                tap_drag: true,
                tap_drag_lock: false,
                tap_buttons: TapButtons::LeftRightMiddle,
                click: ClickMethod::Areas,
                scroll: TouchpadScroll::TwoFinger,
                disable_while_typing: true,
                disable_while_pointing: true,
                middle_emulation: false,
                scroll_factor: 1.0,
            },
            cursor: CursorRules {
                theme: None,
                size: 24,
            },
        }
    }

    pub(crate) fn devices(&self) -> DeviceSettings {
        DeviceSettings {
            mouse: self.mouse.devices(),
            touchpad: self.touchpad.devices(),
        }
    }
}

impl MouseRules {
    fn devices(&self) -> MouseSettings {
        MouseSettings {
            enabled: self.enabled,
            speed: self.speed,
            acceleration: self.acceleration.devices(),
            natural_scroll: self.natural_scroll,
            left_handed: self.left_handed,
            middle_emulation: self.middle_emulation,
            scroll: match self.scroll {
                MouseScroll::Wheel => rikka_compositor_input::MouseScroll::Wheel,
                MouseScroll::Button => rikka_compositor_input::MouseScroll::Button,
            },
            scroll_button: self.scroll_button,
            scroll_button_lock: self.scroll_button_lock,
            rotation: self.rotation,
        }
    }
}

impl TouchpadRules {
    fn devices(&self) -> TouchpadSettings {
        TouchpadSettings {
            used: match self.used {
                TouchpadUse::Always => rikka_compositor_input::TouchpadUse::Always,
                TouchpadUse::Never => rikka_compositor_input::TouchpadUse::Never,
                TouchpadUse::NoMouse => rikka_compositor_input::TouchpadUse::NoMouse,
            },
            speed: self.speed,
            acceleration: self.acceleration.devices(),
            natural_scroll: self.natural_scroll,
            left_handed: self.left_handed,
            tap: self.tap,
            tap_drag: self.tap_drag,
            tap_drag_lock: self.tap_drag_lock,
            tap_buttons: match self.tap_buttons {
                TapButtons::LeftRightMiddle => rikka_compositor_input::TapButtons::LeftRightMiddle,
                TapButtons::LeftMiddleRight => rikka_compositor_input::TapButtons::LeftMiddleRight,
            },
            click: match self.click {
                ClickMethod::Areas => rikka_compositor_input::ClickMethod::Areas,
                ClickMethod::Fingers => rikka_compositor_input::ClickMethod::Fingers,
            },
            scroll: match self.scroll {
                TouchpadScroll::TwoFinger => rikka_compositor_input::TouchpadScroll::TwoFinger,
                TouchpadScroll::Edge => rikka_compositor_input::TouchpadScroll::Edge,
                TouchpadScroll::None => rikka_compositor_input::TouchpadScroll::Off,
            },
            disable_while_typing: self.disable_while_typing,
            disable_while_pointing: self.disable_while_pointing,
            middle_emulation: self.middle_emulation,
        }
    }
}

impl Acceleration {
    fn devices(self) -> rikka_compositor_input::Acceleration {
        match self {
            Self::Adaptive => rikka_compositor_input::Acceleration::Adaptive,
            Self::Flat => rikka_compositor_input::Acceleration::Flat,
        }
    }
}
