use std::ops::RangeInclusive;

use crate::key::Key;
use crate::schema::Schema;

pub const DOMAIN: &str = "rikka.pointer";

pub const MOUSE_ENABLED: &str = "mouse-enabled";
pub const MOUSE_SPEED: &str = "mouse-speed";
pub const MOUSE_ACCELERATION: &str = "mouse-acceleration";
pub const MOUSE_NATURAL_SCROLL: &str = "mouse-natural-scroll";
pub const MOUSE_LEFT_HANDED: &str = "mouse-left-handed";
pub const MOUSE_MIDDLE_EMULATION: &str = "mouse-middle-emulation";
pub const MOUSE_SCROLL_METHOD: &str = "mouse-scroll-method";
pub const MOUSE_SCROLL_BUTTON: &str = "mouse-scroll-button";
pub const MOUSE_SCROLL_BUTTON_LOCK: &str = "mouse-scroll-button-lock";
pub const MOUSE_SCROLL_FACTOR: &str = "mouse-scroll-factor";
pub const MOUSE_ROTATION: &str = "mouse-rotation";

pub const TOUCHPAD_USED: &str = "touchpad-used";
pub const TOUCHPAD_SPEED: &str = "touchpad-speed";
pub const TOUCHPAD_ACCELERATION: &str = "touchpad-acceleration";
pub const TOUCHPAD_NATURAL_SCROLL: &str = "touchpad-natural-scroll";
pub const TOUCHPAD_LEFT_HANDED: &str = "touchpad-left-handed";
pub const TOUCHPAD_TAP: &str = "touchpad-tap";
pub const TOUCHPAD_TAP_DRAG: &str = "touchpad-tap-drag";
pub const TOUCHPAD_TAP_DRAG_LOCK: &str = "touchpad-tap-drag-lock";
pub const TOUCHPAD_TAP_BUTTONS: &str = "touchpad-tap-buttons";
pub const TOUCHPAD_CLICK_METHOD: &str = "touchpad-click-method";
pub const TOUCHPAD_SCROLL_METHOD: &str = "touchpad-scroll-method";
pub const TOUCHPAD_DISABLE_WHILE_TYPING: &str = "touchpad-disable-while-typing";
pub const TOUCHPAD_DISABLE_WHILE_POINTING: &str = "touchpad-disable-while-pointing";
pub const TOUCHPAD_MIDDLE_EMULATION: &str = "touchpad-middle-emulation";
pub const TOUCHPAD_SCROLL_FACTOR: &str = "touchpad-scroll-factor";

pub const CURSOR_THEME: &str = "cursor-theme";
pub const CURSOR_SIZE: &str = "cursor-size";

pub const ACCELERATIONS: [&str; 2] = ["adaptive", "flat"];
pub const MOUSE_SCROLLS: [&str; 2] = ["wheel", "button"];
pub const TOUCHPAD_USES: [&str; 3] = ["always", "never", "no-mouse"];
pub const TAP_BUTTONS: [&str; 2] = ["left-right-middle", "left-middle-right"];
pub const CLICK_METHODS: [&str; 2] = ["areas", "fingers"];
pub const TOUCHPAD_SCROLLS: [&str; 3] = ["two-finger", "edge", "none"];

pub const SPEEDS: RangeInclusive<f64> = -1.0..=1.0;
pub const SCROLL_FACTORS: RangeInclusive<f64> = 0.1..=10.0;
pub const SCROLL_BUTTONS: [i64; 7] = [273, 274, 275, 276, 277, 278, 279];
pub const ROTATIONS: RangeInclusive<i64> = 0..=359;
pub const CURSOR_SIZES: RangeInclusive<i64> = 8..=128;

pub const ENABLED_DEFAULT: bool = true;
pub const SPEED_DEFAULT: f64 = 0.0;
pub const ACCELERATION_DEFAULT: &str = ACCELERATIONS[0];
pub const MOUSE_NATURAL_SCROLL_DEFAULT: bool = false;
pub const LEFT_HANDED_DEFAULT: bool = false;
pub const MIDDLE_EMULATION_DEFAULT: bool = false;
pub const MOUSE_SCROLL_DEFAULT: &str = MOUSE_SCROLLS[0];
pub const SCROLL_BUTTON_DEFAULT: i64 = 274;
pub const SCROLL_BUTTON_LOCK_DEFAULT: bool = false;
pub const SCROLL_FACTOR_DEFAULT: f64 = 1.0;
pub const ROTATION_DEFAULT: i64 = 0;

pub const TOUCHPAD_USED_DEFAULT: &str = TOUCHPAD_USES[0];
pub const TOUCHPAD_NATURAL_SCROLL_DEFAULT: bool = true;
pub const TAP_DEFAULT: bool = true;
pub const TAP_DRAG_DEFAULT: bool = true;
pub const TAP_DRAG_LOCK_DEFAULT: bool = false;
pub const TAP_BUTTONS_DEFAULT: &str = TAP_BUTTONS[0];
pub const CLICK_METHOD_DEFAULT: &str = CLICK_METHODS[0];
pub const TOUCHPAD_SCROLL_DEFAULT: &str = TOUCHPAD_SCROLLS[0];
pub const DISABLE_WHILE_TYPING_DEFAULT: bool = true;
pub const DISABLE_WHILE_POINTING_DEFAULT: bool = true;

pub const INHERITED: &str = "";
pub const CURSOR_SIZE_DEFAULT: i64 = 24;

#[must_use]
pub fn schema() -> Schema {
    Schema::new(DOMAIN)
        .summary("How the session's pointers behave, and what its cursor looks like")
        .key(
            Key::bool(MOUSE_ENABLED, ENABLED_DEFAULT)
                .summary("Whether a mouse or trackball reports at all"),
        )
        .key(
            Key::float(MOUSE_SPEED, SPEED_DEFAULT)
                .summary("How far the pointer travels for a given movement, from -1 to 1")
                .range(SPEEDS),
        )
        .key(
            Key::string(MOUSE_ACCELERATION, ACCELERATION_DEFAULT)
                .summary("Whether pointer travel depends on how fast the mouse is moved")
                .choices(ACCELERATIONS),
        )
        .key(
            Key::bool(MOUSE_NATURAL_SCROLL, MOUSE_NATURAL_SCROLL_DEFAULT)
                .summary("Whether the page follows the wheel rather than the scrollbar"),
        )
        .key(
            Key::bool(MOUSE_LEFT_HANDED, LEFT_HANDED_DEFAULT)
                .summary("Whether the left and right buttons are swapped"),
        )
        .key(
            Key::bool(MOUSE_MIDDLE_EMULATION, MIDDLE_EMULATION_DEFAULT)
                .summary("Whether pressing both buttons at once is a middle click"),
        )
        .key(
            Key::string(MOUSE_SCROLL_METHOD, MOUSE_SCROLL_DEFAULT)
                .summary("Whether the wheel scrolls, or moving the mouse with a button held does")
                .choices(MOUSE_SCROLLS),
        )
        .key(
            Key::int(MOUSE_SCROLL_BUTTON, SCROLL_BUTTON_DEFAULT)
                .summary("The evdev button held to scroll by moving the mouse")
                .choices(SCROLL_BUTTONS),
        )
        .key(
            Key::bool(MOUSE_SCROLL_BUTTON_LOCK, SCROLL_BUTTON_LOCK_DEFAULT)
                .summary("Whether the scroll button latches instead of being held down"),
        )
        .key(
            Key::float(MOUSE_SCROLL_FACTOR, SCROLL_FACTOR_DEFAULT)
                .summary("What one notch of the wheel is multiplied by")
                .range(SCROLL_FACTORS),
        )
        .key(
            Key::int(MOUSE_ROTATION, ROTATION_DEFAULT)
                .summary("How far the device is turned from upright, in degrees")
                .range(ROTATIONS),
        )
        .key(
            Key::string(TOUCHPAD_USED, TOUCHPAD_USED_DEFAULT)
                .summary("When the touchpad reports: always, never, or only with no mouse plugged in")
                .choices(TOUCHPAD_USES),
        )
        .key(
            Key::float(TOUCHPAD_SPEED, SPEED_DEFAULT)
                .summary("How far the pointer travels for a given movement, from -1 to 1")
                .range(SPEEDS),
        )
        .key(
            Key::string(TOUCHPAD_ACCELERATION, ACCELERATION_DEFAULT)
                .summary("Whether pointer travel depends on how fast the finger moves")
                .choices(ACCELERATIONS),
        )
        .key(
            Key::bool(TOUCHPAD_NATURAL_SCROLL, TOUCHPAD_NATURAL_SCROLL_DEFAULT)
                .summary("Whether the page follows the fingers rather than the scrollbar"),
        )
        .key(
            Key::bool(TOUCHPAD_LEFT_HANDED, LEFT_HANDED_DEFAULT)
                .summary("Whether the left and right buttons are swapped"),
        )
        .key(Key::bool(TOUCHPAD_TAP, TAP_DEFAULT).summary("Whether a tap is a click"))
        .key(
            Key::bool(TOUCHPAD_TAP_DRAG, TAP_DRAG_DEFAULT)
                .summary("Whether a tap followed by a drag moves what was tapped"),
        )
        .key(
            Key::bool(TOUCHPAD_TAP_DRAG_LOCK, TAP_DRAG_LOCK_DEFAULT)
                .summary("Whether a drag survives lifting the finger for a moment"),
        )
        .key(
            Key::string(TOUCHPAD_TAP_BUTTONS, TAP_BUTTONS_DEFAULT)
                .summary("Which button a two- or three-finger tap is")
                .choices(TAP_BUTTONS),
        )
        .key(
            Key::string(TOUCHPAD_CLICK_METHOD, CLICK_METHOD_DEFAULT)
                .summary("Whether where the pad is pressed decides the button, or how many fingers are down")
                .choices(CLICK_METHODS),
        )
        .key(
            Key::string(TOUCHPAD_SCROLL_METHOD, TOUCHPAD_SCROLL_DEFAULT)
                .summary("What scrolls: two fingers, the edge of the pad, or nothing")
                .choices(TOUCHPAD_SCROLLS),
        )
        .key(
            Key::bool(TOUCHPAD_DISABLE_WHILE_TYPING, DISABLE_WHILE_TYPING_DEFAULT)
                .summary("Whether the touchpad is ignored while the keyboard is in use"),
        )
        .key(
            Key::bool(TOUCHPAD_DISABLE_WHILE_POINTING, DISABLE_WHILE_POINTING_DEFAULT)
                .summary("Whether the touchpad is ignored while the trackpoint is in use"),
        )
        .key(
            Key::bool(TOUCHPAD_MIDDLE_EMULATION, MIDDLE_EMULATION_DEFAULT)
                .summary("Whether pressing both buttons at once is a middle click"),
        )
        .key(
            Key::float(TOUCHPAD_SCROLL_FACTOR, SCROLL_FACTOR_DEFAULT)
                .summary("What a finger's worth of scrolling is multiplied by")
                .range(SCROLL_FACTORS),
        )
        .key(
            Key::string(CURSOR_THEME, INHERITED)
                .summary("The XCursor theme to draw the pointer from; empty follows the machine"),
        )
        .key(
            Key::int(CURSOR_SIZE, CURSOR_SIZE_DEFAULT)
                .summary("How large the pointer is drawn, in logical pixels")
                .range(CURSOR_SIZES),
        )
}
