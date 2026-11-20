use std::io::{Seek as _, Write as _};

use rikka_input::Error;
use rikka_input::xkb::{Keyboard, SHIFT, key, key_with_text, keysym};
use rikka_types::{Key, KeyCode, KeyLocation, Modifiers};
use xkeysym::Keysym;

const KEYMAP: &str = r#"xkb_keymap {
    xkb_keycodes "test" { minimum = 8; maximum = 255; <AC01> = 38; <LFSH> = 50; };
    xkb_types "test" {
        type "ONE_LEVEL" { modifiers = none; level_name[Level1] = "Any"; };
        type "TWO_LEVEL" {
            modifiers = Shift;
            map[Shift] = Level2;
            level_name[Level1] = "Base";
            level_name[Level2] = "Shift";
        };
    };
    xkb_compatibility "test" {
        interpret Shift_L { action = SetMods(modifiers = Shift); };
    };
    xkb_symbols "test" {
        key <AC01> { type = "TWO_LEVEL", [ a, A ] };
        key <LFSH> { [ Shift_L ] };
        modifier_map Shift { <LFSH> };
    };
};"#;

const KEY_A: u32 = 30;

fn keymap_file(text: &[u8]) -> std::fs::File {
    let mut file = tempfile::tempfile().expect("a file to hold the keymap");
    file.write_all(text).expect("the keymap written");
    file.write_all(&[0]).expect("the keymap terminated");
    file.rewind().expect("the keymap rewound");
    file
}

#[test]
fn named_keys_round_trip_keysyms() {
    let named = [
        Key::Enter,
        Key::Escape,
        Key::Tab,
        Key::Backspace,
        Key::Delete,
        Key::Space,
        Key::ArrowUp,
        Key::PageDown,
        Key::F(1),
        Key::F(12),
        Key::ShiftLeft,
        Key::ShiftRight,
        Key::ControlRight,
        Key::AltLeft,
        Key::MetaRight,
        Key::BrowserBack,
        Key::Copy,
        Key::Paste,
        Key::Undo,
        Key::Find,
    ];
    for expected in named {
        let keysym = keysym(expected).expect("a named key has a keysym");
        assert_eq!(key(keysym), expected, "{keysym:?}");
    }
    assert_eq!(key(Keysym::KP_Enter), Key::Enter);
    assert_eq!(key(Keysym::ISO_Left_Tab), Key::Tab);
    assert_eq!(key(Keysym::VoidSymbol), Key::Unidentified);
}

#[test]
fn text_overrides_keysym_except_control() {
    assert_eq!(key_with_text(Keysym::a, Some("ä")), Key::Char('ä'));
    assert_eq!(key_with_text(Keysym::c, Some("\u{3}")), Key::Char('c'));
    assert_eq!(key_with_text(Keysym::c, None), Key::Char('c'));
    assert_eq!(key_with_text(Keysym::VoidSymbol, None), Key::Unidentified);
}

#[test]
fn key_resolves_through_keymap_and_modifiers() {
    let mut keyboard = Keyboard::new();
    assert_eq!(
        keyboard.press(KEY_A),
        None,
        "nothing reads without a keymap"
    );

    let file = keymap_file(KEYMAP.as_bytes());
    unsafe { keyboard.load(file.into(), KEYMAP.len() + 1) }.expect("the keymap compiles");
    assert!(keyboard.key_repeats(KEY_A));

    let plain = keyboard.press(KEY_A).expect("a keymap reads the key");
    assert_eq!(plain.key(), Key::Char('a'));
    assert_eq!(plain.code(), KeyCode::from_evdev(KEY_A));
    assert_eq!(plain.location(), KeyLocation::Standard);

    assert_eq!(
        keyboard.set_modifiers(SHIFT, 0, 0, 0),
        Some(Modifiers::SHIFT)
    );
    assert_eq!(
        keyboard.press(KEY_A).map(|stroke| stroke.key()),
        Some(Key::Char('A'))
    );
    let mut stroke = plain;
    keyboard.refresh_text(&mut stroke);
    assert_eq!(stroke.key(), Key::Char('A'));
}

const DEAD_KEYMAP: &str = r#"xkb_keymap {
    xkb_keycodes "test" { minimum = 8; maximum = 255; <AE12> = 21; <AD03> = 26; <LFSH> = 50; };
    xkb_types "test" {
        type "ONE_LEVEL" { modifiers = none; level_name[Level1] = "Any"; };
        type "TWO_LEVEL" {
            modifiers = Shift;
            map[Shift] = Level2;
            level_name[Level1] = "Base";
            level_name[Level2] = "Shift";
        };
    };
    xkb_compatibility "test" {
        interpret Shift_L { action = SetMods(modifiers = Shift); };
    };
    xkb_symbols "test" {
        key <AE12> { type = "ONE_LEVEL", [ dead_acute ] };
        key <AD03> { type = "TWO_LEVEL", [ e, E ] };
        key <LFSH> { [ Shift_L ] };
        modifier_map Shift { <LFSH> };
    };
};"#;

const KEY_DEAD_ACUTE: u32 = 13;

const KEY_E: u32 = 18;

#[test]
fn dead_key_composes_across_modifier_change() {
    let mut keyboard = Keyboard::new();
    let file = keymap_file(DEAD_KEYMAP.as_bytes());
    unsafe { keyboard.load(file.into(), DEAD_KEYMAP.len() + 1) }.expect("the keymap compiles");

    let mut dead = keyboard.press(KEY_DEAD_ACUTE).expect("the dead key reads");
    keyboard.set_modifiers(SHIFT, 0, 0, 0);
    keyboard.refresh_text(&mut dead);
    let composed = keyboard.press(KEY_E).expect("the next key reads");
    assert_eq!(composed.key(), Key::Char('É'));

    keyboard.set_modifiers(0, 0, 0, 0);
    let plain = keyboard
        .press(KEY_E)
        .expect("a key after the composition reads");
    assert_eq!(plain.key(), Key::Char('e'));
}

#[test]
fn rejects_invalid_keymap() {
    let text = b"xkb_keymap { nonsense };";
    let mut keyboard = Keyboard::new();
    assert_eq!(
        unsafe { keyboard.load(keymap_file(text).into(), text.len() + 1) },
        Err(Error::InvalidKeymap)
    );
    assert!(!keyboard.is_ready());
}
