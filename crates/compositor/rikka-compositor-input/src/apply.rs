use input::{
    AccelProfile, ClickMethod as LibinputClick, Device, DeviceConfigError, DeviceConfigResult,
    ScrollButtonLockState, ScrollMethod, SendEventsMode, TapButtonMap,
};

use crate::device::{DeviceId, PointerKind};
use crate::plan::{Adjustment, ScrollWay, SendMode, plan};
use crate::report::{ConfigReport, SettingError};
use crate::settings::{Acceleration, ClickMethod, DeviceSettings, TapButtons};

pub(crate) fn configure(
    device: &mut Device,
    id: DeviceId,
    kind: PointerKind,
    settings: &DeviceSettings,
) -> ConfigReport {
    let rejected = plan(kind, settings)
        .into_iter()
        .filter_map(|adjustment| {
            let setting_error = if offers(device, adjustment) {
                adjust(device, adjustment).err().map(setting_error)
            } else {
                (!adjustment.is_neutral()).then_some(SettingError::Unsupported)
            };
            setting_error.map(|setting_error| (adjustment.setting(), setting_error))
        })
        .collect();
    ConfigReport {
        device: id,
        rejected,
    }
}

fn offers(device: &Device, adjustment: Adjustment) -> bool {
    match adjustment {
        Adjustment::Send(SendMode::Enabled) => true,
        Adjustment::Send(mode) => device.config_send_events_modes().contains(send_mode(mode)),
        Adjustment::Speed(_) => device.config_accel_is_available(),
        Adjustment::Acceleration(acceleration) => device
            .config_accel_profiles()
            .contains(&profile(acceleration)),
        Adjustment::NaturalScroll(_) => device.config_scroll_has_natural_scroll(),
        Adjustment::LeftHanded(_) => device.config_left_handed_is_available(),
        Adjustment::MiddleEmulation(_) => device.config_middle_emulation_is_available(),
        Adjustment::Scroll(ScrollWay::Nothing) => true,
        Adjustment::Scroll(way) => device.config_scroll_methods().contains(&scroll_method(way)),
        Adjustment::ScrollButton(_) | Adjustment::ScrollButtonLock(_) => device
            .config_scroll_methods()
            .contains(&ScrollMethod::OnButtonDown),
        Adjustment::Rotation(_) => device.config_rotation_is_available(),
        Adjustment::Tap(_)
        | Adjustment::TapDrag(_)
        | Adjustment::TapDragLock(_)
        | Adjustment::TapButtons(_) => device.config_tap_finger_count() > 0,
        Adjustment::Click(method) => device
            .config_click_methods()
            .contains(&click_method(method)),
        Adjustment::DisableWhileTyping(_) => device.config_dwt_is_available(),
        Adjustment::DisableWhilePointing(_) => device.config_dwtp_is_available(),
    }
}

fn adjust(device: &mut Device, adjustment: Adjustment) -> DeviceConfigResult {
    match adjustment {
        Adjustment::Send(mode) => device.config_send_events_set_mode(send_mode(mode)),
        Adjustment::Speed(speed) => device.config_accel_set_speed(speed),
        Adjustment::Acceleration(acceleration) => {
            device.config_accel_set_profile(profile(acceleration))
        }
        Adjustment::NaturalScroll(on) => device.config_scroll_set_natural_scroll_enabled(on),
        Adjustment::LeftHanded(on) => device.config_left_handed_set(on),
        Adjustment::MiddleEmulation(on) => device.config_middle_emulation_set_enabled(on),
        Adjustment::Scroll(way) => device.config_scroll_set_method(scroll_method(way)),
        Adjustment::ScrollButton(button) => device.config_scroll_set_button(button),
        Adjustment::ScrollButtonLock(on) => device.config_scroll_set_button_lock(if on {
            ScrollButtonLockState::Enabled
        } else {
            ScrollButtonLockState::Disabled
        }),
        Adjustment::Rotation(angle) => device.config_rotation_set_angle(angle),
        Adjustment::Tap(on) => device.config_tap_set_enabled(on),
        Adjustment::TapDrag(on) => device.config_tap_set_drag_enabled(on),
        Adjustment::TapDragLock(on) => device.config_tap_set_drag_lock_enabled(on),
        Adjustment::TapButtons(map) => device.config_tap_set_button_map(match map {
            TapButtons::LeftRightMiddle => TapButtonMap::LeftRightMiddle,
            TapButtons::LeftMiddleRight => TapButtonMap::LeftMiddleRight,
        }),
        Adjustment::Click(method) => device.config_click_set_method(click_method(method)),
        Adjustment::DisableWhileTyping(on) => device.config_dwt_set_enabled(on),
        Adjustment::DisableWhilePointing(on) => device.config_dwtp_set_enabled(on),
    }
}

fn setting_error(error: DeviceConfigError) -> SettingError {
    match error {
        DeviceConfigError::Unsupported => SettingError::Unsupported,
        DeviceConfigError::Invalid => SettingError::Invalid,
    }
}

fn send_mode(mode: SendMode) -> SendEventsMode {
    match mode {
        SendMode::Enabled => SendEventsMode::ENABLED,
        SendMode::Disabled => SendEventsMode::DISABLED,
        SendMode::DisabledOnExternalMouse => SendEventsMode::DISABLED_ON_EXTERNAL_MOUSE,
    }
}

fn profile(acceleration: Acceleration) -> AccelProfile {
    match acceleration {
        Acceleration::Adaptive => AccelProfile::Adaptive,
        Acceleration::Flat => AccelProfile::Flat,
    }
}

fn scroll_method(way: ScrollWay) -> ScrollMethod {
    match way {
        ScrollWay::Nothing => ScrollMethod::NoScroll,
        ScrollWay::TwoFinger => ScrollMethod::TwoFinger,
        ScrollWay::Edge => ScrollMethod::Edge,
        ScrollWay::Button => ScrollMethod::OnButtonDown,
    }
}

fn click_method(method: ClickMethod) -> LibinputClick {
    match method {
        ClickMethod::Areas => LibinputClick::ButtonAreas,
        ClickMethod::Fingers => LibinputClick::Clickfinger,
    }
}
