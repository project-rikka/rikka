use rikka_input::win32::{
    VK_CONTROL, VK_F1, VK_F24, VK_KANA, VK_MENU, VK_NEXT, VK_RETURN, VK_SHIFT, WHEEL_PAGE,
    key_code, location, named_key, wheel_offset,
};
use rikka_types::{Key, KeyCode, KeyLocation, Point, SCROLL_LINE};

#[test]
fn scan_codes_become_evdev_codes() {
    for (scan, extended, evdev) in [
        (0x01, false, 1),
        (0x1E, false, 30),
        (0x2F, false, 47),
        (0x1D, false, 29),
        (0x1D, true, 97),
        (0x38, true, 100),
        (0x1C, true, 96),
        (0x35, true, 98),
        (0x37, true, 99),
        (0x45, true, 69),
        (0x45, false, 119),
        (0x47, true, 102),
        (0x48, true, 103),
        (0x53, true, 111),
        (0x5B, true, 125),
        (0x5D, true, 127),
        (0x57, false, 87),
        (0x58, false, 88),
        (0x64, false, 183),
        (0x6E, false, 193),
        (0x76, false, 194),
        (0x70, false, 93),
        (0x73, false, 89),
        (0x79, false, 92),
        (0x7B, false, 94),
        (0x7D, false, 124),
        (0x6A, true, 158),
    ] {
        assert_eq!(
            key_code(scan, extended),
            KeyCode::from_evdev(evdev),
            "scan {scan:#x} extended {extended}"
        );
    }
    assert_eq!(key_code(0x2A, true), KeyCode::UNKNOWN);
}

#[test]
fn keypad_keys_are_located_on_the_keypad() {
    for (scan, extended, place) in [
        (0x4F, false, KeyLocation::Keypad),
        (0x4F, true, KeyLocation::Standard),
        (0x1C, true, KeyLocation::Keypad),
        (0x1C, false, KeyLocation::Standard),
        (0x35, true, KeyLocation::Keypad),
        (0x35, false, KeyLocation::Standard),
        (0x37, false, KeyLocation::Keypad),
    ] {
        assert_eq!(location(scan, extended), place, "scan {scan:#x}");
    }
}

#[test]
fn virtual_keys_name_sided_and_special_keys() {
    for (virtual_key, scan, extended, key) in [
        (VK_SHIFT, 0x2A, false, Some(Key::ShiftLeft)),
        (VK_SHIFT, 0x36, false, Some(Key::ShiftRight)),
        (VK_CONTROL, 0x1D, true, Some(Key::ControlRight)),
        (VK_MENU, 0x38, false, Some(Key::AltLeft)),
        (VK_MENU, 0x38, true, Some(Key::AltRight)),
        (VK_RETURN, 0x1C, true, Some(Key::Enter)),
        (VK_NEXT, 0x51, true, Some(Key::PageDown)),
        (VK_F1, 0x3B, false, Some(Key::F(1))),
        (VK_F24, 0x76, false, Some(Key::F(24))),
        (VK_KANA, 0x70, false, Some(Key::KanaMode)),
        (VK_KANA, 0x72, false, Some(Key::HangulMode)),
        (0x41, 0x1E, false, None),
    ] {
        assert_eq!(
            named_key(virtual_key, scan, extended),
            key,
            "vk {virtual_key:#x}"
        );
    }
}

#[test]
fn wheel_notches_scroll_like_wayland_wheels() {
    for (delta, lines, horizontal, offset) in [
        (120, 3, false, Point::new(0.0, -SCROLL_LINE)),
        (-120, 3, false, Point::new(0.0, SCROLL_LINE)),
        (60, 3, false, Point::new(0.0, -SCROLL_LINE / 2.0)),
        (120, 6, false, Point::new(0.0, -2.0 * SCROLL_LINE)),
        (120, WHEEL_PAGE, false, Point::new(0.0, -SCROLL_LINE)),
        (120, 3, true, Point::new(SCROLL_LINE, 0.0)),
    ] {
        assert_eq!(
            wheel_offset(delta, lines, horizontal),
            offset,
            "delta {delta}"
        );
    }
}
