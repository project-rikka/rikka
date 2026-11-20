use rikka_input::evdev::{
    BTN_BACK, BTN_EXTRA, BTN_FORWARD, BTN_LEFT, BTN_SIDE, BTN_STYLUS, BTN_STYLUS2, BTN_STYLUS3,
    button_code, key_code, pointer_button, tool_button,
};
use rikka_types::{Key, PointerButton};

#[test]
fn button_codes_round_trip() {
    for button in [
        PointerButton::Primary,
        PointerButton::Secondary,
        PointerButton::Middle,
        PointerButton::Back,
        PointerButton::Forward,
        PointerButton::Other(0x118),
    ] {
        assert_eq!(pointer_button(button_code(button)), button);
    }
    assert_eq!(pointer_button(BTN_SIDE), PointerButton::Back);
    assert_eq!(pointer_button(BTN_BACK), PointerButton::Back);
    assert_eq!(pointer_button(BTN_EXTRA), PointerButton::Forward);
    assert_eq!(pointer_button(BTN_FORWARD), PointerButton::Forward);
}

#[test]
fn stylus_barrel_buttons_follow_desktop_convention() {
    for (code, button) in [
        (BTN_STYLUS, PointerButton::Secondary),
        (BTN_STYLUS2, PointerButton::Middle),
        (BTN_STYLUS3, PointerButton::Back),
        (BTN_LEFT, PointerButton::Primary),
    ] {
        assert_eq!(tool_button(code), button);
    }
}

#[test]
fn key_code_matches_keyboard_position() {
    assert_eq!(key_code(Key::Char('v')), Some(47));
    assert_eq!(key_code(Key::Char('V')), Some(47));
    assert_eq!(key_code(Key::Enter), Some(28));
    assert_eq!(key_code(Key::F(24)), Some(194));
    assert_eq!(key_code(Key::Unidentified), None);
    assert_eq!(key_code(Key::F(25)), None);
    assert_eq!(key_code(Key::Char('あ')), None);
}
