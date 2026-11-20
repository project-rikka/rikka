use crate::device::PointerKind;
use crate::report::Setting;
use crate::settings::{
    Acceleration, ClickMethod, DeviceSettings, MouseScroll, MouseSettings, TapButtons,
    TouchpadScroll, TouchpadSettings, TouchpadUse,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SendMode {
    Enabled,
    Disabled,
    DisabledOnExternalMouse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScrollWay {
    Nothing,
    TwoFinger,
    Edge,
    Button,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Adjustment {
    Send(SendMode),
    Speed(f64),
    Acceleration(Acceleration),
    NaturalScroll(bool),
    LeftHanded(bool),
    MiddleEmulation(bool),
    Scroll(ScrollWay),
    ScrollButton(u32),
    ScrollButtonLock(bool),
    Rotation(u32),
    Tap(bool),
    TapDrag(bool),
    TapDragLock(bool),
    TapButtons(TapButtons),
    Click(ClickMethod),
    DisableWhileTyping(bool),
    DisableWhilePointing(bool),
}

impl Adjustment {
    pub(crate) fn setting(self) -> Setting {
        match self {
            Self::Send(_) => Setting::Enabled,
            Self::Speed(_) => Setting::Speed,
            Self::Acceleration(_) => Setting::Acceleration,
            Self::NaturalScroll(_) => Setting::NaturalScroll,
            Self::LeftHanded(_) => Setting::LeftHanded,
            Self::MiddleEmulation(_) => Setting::MiddleEmulation,
            Self::Scroll(_) => Setting::ScrollMethod,
            Self::ScrollButton(_) => Setting::ScrollButton,
            Self::ScrollButtonLock(_) => Setting::ScrollButtonLock,
            Self::Rotation(_) => Setting::Rotation,
            Self::Tap(_) => Setting::Tap,
            Self::TapDrag(_) => Setting::TapDrag,
            Self::TapDragLock(_) => Setting::TapDragLock,
            Self::TapButtons(_) => Setting::TapButtons,
            Self::Click(_) => Setting::ClickMethod,
            Self::DisableWhileTyping(_) => Setting::DisableWhileTyping,
            Self::DisableWhilePointing(_) => Setting::DisableWhilePointing,
        }
    }

    pub(crate) fn is_neutral(self) -> bool {
        match self {
            Self::Send(mode) => mode == SendMode::Enabled,
            Self::Speed(speed) => speed == 0.0,
            Self::Acceleration(profile) => profile == Acceleration::Flat,
            Self::Scroll(way) => way == ScrollWay::Nothing,
            Self::Rotation(angle) => angle == 0,
            Self::TapButtons(map) => map == TapButtons::LeftRightMiddle,
            Self::Click(method) => method == ClickMethod::Areas,
            Self::ScrollButton(_) => false,
            Self::NaturalScroll(on)
            | Self::LeftHanded(on)
            | Self::MiddleEmulation(on)
            | Self::ScrollButtonLock(on)
            | Self::Tap(on)
            | Self::TapDrag(on)
            | Self::TapDragLock(on)
            | Self::DisableWhileTyping(on)
            | Self::DisableWhilePointing(on) => !on,
        }
    }
}

pub(crate) fn plan(kind: PointerKind, settings: &DeviceSettings) -> Vec<Adjustment> {
    match kind {
        PointerKind::Mouse => mouse(&settings.mouse),
        PointerKind::Touchpad => touchpad(&settings.touchpad),
    }
}

fn mouse(settings: &MouseSettings) -> Vec<Adjustment> {
    let mut plan = vec![
        Adjustment::Send(if settings.enabled {
            SendMode::Enabled
        } else {
            SendMode::Disabled
        }),
        Adjustment::Speed(settings.speed),
        Adjustment::Acceleration(settings.acceleration),
        Adjustment::NaturalScroll(settings.natural_scroll),
        Adjustment::LeftHanded(settings.left_handed),
        Adjustment::MiddleEmulation(settings.middle_emulation),
        Adjustment::Rotation(settings.rotation),
    ];
    match settings.scroll {
        MouseScroll::Wheel => plan.push(Adjustment::Scroll(ScrollWay::Nothing)),
        MouseScroll::Button => plan.extend([
            Adjustment::Scroll(ScrollWay::Button),
            Adjustment::ScrollButton(settings.scroll_button),
            Adjustment::ScrollButtonLock(settings.scroll_button_lock),
        ]),
    }
    plan
}

fn touchpad(settings: &TouchpadSettings) -> Vec<Adjustment> {
    vec![
        Adjustment::Send(match settings.used {
            TouchpadUse::Always => SendMode::Enabled,
            TouchpadUse::Never => SendMode::Disabled,
            TouchpadUse::NoMouse => SendMode::DisabledOnExternalMouse,
        }),
        Adjustment::Speed(settings.speed),
        Adjustment::Acceleration(settings.acceleration),
        Adjustment::NaturalScroll(settings.natural_scroll),
        Adjustment::LeftHanded(settings.left_handed),
        Adjustment::Tap(settings.tap),
        Adjustment::TapDrag(settings.tap_drag),
        Adjustment::TapDragLock(settings.tap_drag_lock),
        Adjustment::TapButtons(settings.tap_buttons),
        Adjustment::Click(settings.click),
        Adjustment::Scroll(match settings.scroll {
            TouchpadScroll::TwoFinger => ScrollWay::TwoFinger,
            TouchpadScroll::Edge => ScrollWay::Edge,
            TouchpadScroll::Off => ScrollWay::Nothing,
        }),
        Adjustment::DisableWhileTyping(settings.disable_while_typing),
        Adjustment::DisableWhilePointing(settings.disable_while_pointing),
        Adjustment::MiddleEmulation(settings.middle_emulation),
    ]
}
