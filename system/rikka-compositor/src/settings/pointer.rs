use rikka_settings::Schema;
use rikka_settings::seam::Settings;
use rikka_settings::session::pointer::{self as keys, *};

use crate::input::{
    Acceleration, ClickMethod, CursorRules, MouseRules, MouseScroll, PointerRules, TapButtons,
    TouchpadRules, TouchpadScroll, TouchpadUse,
};

pub(crate) use rikka_settings::session::pointer::DOMAIN;

pub(crate) fn schema() -> Schema {
    keys::schema()
}

pub(crate) fn rules(settings: &Settings) -> PointerRules {
    let PointerRules {
        mouse,
        touchpad,
        cursor,
    } = PointerRules::declared();
    PointerRules {
        mouse: MouseRules {
            enabled: settings.read_or(MOUSE_ENABLED, mouse.enabled),
            speed: settings.read_or(MOUSE_SPEED, mouse.speed),
            acceleration: named(
                settings,
                MOUSE_ACCELERATION,
                Acceleration::from_name,
                mouse.acceleration,
            ),
            natural_scroll: settings.read_or(MOUSE_NATURAL_SCROLL, mouse.natural_scroll),
            left_handed: settings.read_or(MOUSE_LEFT_HANDED, mouse.left_handed),
            middle_emulation: settings.read_or(MOUSE_MIDDLE_EMULATION, mouse.middle_emulation),
            scroll: named(
                settings,
                MOUSE_SCROLL_METHOD,
                MouseScroll::from_name,
                mouse.scroll,
            ),
            scroll_button: settings.read_or(MOUSE_SCROLL_BUTTON, mouse.scroll_button),
            scroll_button_lock: settings
                .read_or(MOUSE_SCROLL_BUTTON_LOCK, mouse.scroll_button_lock),
            scroll_factor: settings.read_or(MOUSE_SCROLL_FACTOR, mouse.scroll_factor),
            rotation: settings.read_or(MOUSE_ROTATION, mouse.rotation),
        },
        touchpad: TouchpadRules {
            used: named(
                settings,
                TOUCHPAD_USED,
                TouchpadUse::from_name,
                touchpad.used,
            ),
            speed: settings.read_or(TOUCHPAD_SPEED, touchpad.speed),
            acceleration: named(
                settings,
                TOUCHPAD_ACCELERATION,
                Acceleration::from_name,
                touchpad.acceleration,
            ),
            natural_scroll: settings.read_or(TOUCHPAD_NATURAL_SCROLL, touchpad.natural_scroll),
            left_handed: settings.read_or(TOUCHPAD_LEFT_HANDED, touchpad.left_handed),
            tap: settings.read_or(TOUCHPAD_TAP, touchpad.tap),
            tap_drag: settings.read_or(TOUCHPAD_TAP_DRAG, touchpad.tap_drag),
            tap_drag_lock: settings.read_or(TOUCHPAD_TAP_DRAG_LOCK, touchpad.tap_drag_lock),
            tap_buttons: named(
                settings,
                TOUCHPAD_TAP_BUTTONS,
                TapButtons::from_name,
                touchpad.tap_buttons,
            ),
            click: named(
                settings,
                TOUCHPAD_CLICK_METHOD,
                ClickMethod::from_name,
                touchpad.click,
            ),
            scroll: named(
                settings,
                TOUCHPAD_SCROLL_METHOD,
                TouchpadScroll::from_name,
                touchpad.scroll,
            ),
            disable_while_typing: settings
                .read_or(TOUCHPAD_DISABLE_WHILE_TYPING, touchpad.disable_while_typing),
            disable_while_pointing: settings.read_or(
                TOUCHPAD_DISABLE_WHILE_POINTING,
                touchpad.disable_while_pointing,
            ),
            middle_emulation: settings
                .read_or(TOUCHPAD_MIDDLE_EMULATION, touchpad.middle_emulation),
            scroll_factor: settings.read_or(TOUCHPAD_SCROLL_FACTOR, touchpad.scroll_factor),
        },
        cursor: CursorRules {
            theme: settings
                .read::<String>(CURSOR_THEME)
                .filter(|theme| theme != INHERITED),
            size: settings.read_or(CURSOR_SIZE, cursor.size),
        },
    }
}

fn named<T>(settings: &Settings, setting: &str, parse: fn(&str) -> Option<T>, declared: T) -> T {
    let Some(value) = settings.read::<String>(setting) else {
        return declared;
    };
    parse(&value).unwrap_or_else(|| {
        tracing::warn!("unknown value for `{DOMAIN}/{setting}`: {value}");
        declared
    })
}
