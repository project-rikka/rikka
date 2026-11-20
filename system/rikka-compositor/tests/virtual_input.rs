use rikka_compositor::Extent;
use rikka_input::Chord;
use rikka_input::evdev::{KEY_LEFTCTRL, KEY_LEFTSHIFT};
use rikka_input::xkb::Keyboard;
use rikka_types::{Key, Modifiers};
use rikka_virtual_input::{AbsolutePosition, Error, KeyCode, Seat};
use rikka_wayland_client::wl_pointer::{Axis, AxisSource};
use rikka_wayland_client::zwlr_layer_shell_v1::Layer;
use rikka_wayland_client::zwlr_layer_surface_v1::Anchor;

use crate::common::client::Client;
use crate::common::input::A;
use crate::common::shell::Scrolled;
use crate::common::{Harness, mapped};

const EXTENT: Extent = Extent {
    width: 320,
    height: 240,
};

const GREEN: [u8; 4] = [0, 255, 0, 255];
const OUTPUT: &str = "headless-0";
const GERMAN: &str = "[[sources]]\nlayout = \"de\"\n";

const Y: u32 = 21;
const CAPS_LOCK: u32 = 58;

fn key(code: u32) -> KeyCode {
    KeyCode::from_evdev(code)
}

fn count(codes: &[u32], code: u32) -> usize {
    codes.iter().filter(|recorded| **recorded == code).count()
}

fn focused_window(client: &mut Client) {
    let window = mapped(client, 1, (60, 60), GREEN);
    let id = window.surface.id();
    client.until("keyboard focus on window", |recorded| {
        recorded.shell.keyboard_on == Some(id)
    });
}

fn covering_layer(client: &mut Client) {
    let size = (EXTENT.width, EXTENT.height);
    let (surface, _) = client.layer_on(Layer::Top, 1, Anchor::TOP | Anchor::LEFT, size, 0);
    let pixels = client.pixels(size.0 as i32, size.1 as i32, 1);
    pixels.fill(0..size.1 as i32, GREEN);
    client.show(&surface, &pixels);
}

fn typed(client: &Client, codes: &[u32]) -> Vec<Key> {
    let shell = &client.received.shell;
    let (fd, size) = shell.keymaps.last().expect("keymap received");
    let mut keyboard = Keyboard::new();
    unsafe { keyboard.load(fd.try_clone().unwrap(), *size as usize) }.unwrap();
    shell
        .strokes
        .iter()
        .filter(|(code, _)| codes.contains(code))
        .map(|(code, mods)| {
            keyboard.set_modifiers(mods.depressed, mods.latched, mods.locked, mods.group);
            keyboard.keystroke(*code).unwrap().key()
        })
        .collect()
}

#[test]
fn physical_keyboard_types_with_session_keymap() {
    let harness = Harness::full(EXTENT, &[("rikka.ime", GERMAN)]);
    let mut client = harness.client();
    let _devices = client.devices();
    focused_window(&mut client);

    let seat = Seat::connect_to(harness.socket()).unwrap();
    let keyboard = seat.physical_keyboard().unwrap();
    for (code, pressed) in [
        (Y, true),
        (Y, false),
        (KEY_LEFTSHIFT, true),
        (Y, true),
        (Y, false),
        (KEY_LEFTSHIFT, false),
        (CAPS_LOCK, true),
        (CAPS_LOCK, false),
        (A, true),
        (A, false),
    ] {
        match pressed {
            true => keyboard.press(key(code)).unwrap(),
            false => keyboard.release(key(code)).unwrap(),
        }
    }
    client.until("typed keys", |recorded| recorded.shell.lifted.contains(&A));
    harness.press(&[Y]);
    client.until("person typed", |recorded| {
        count(&recorded.shell.lifted, Y) == 3
    });

    assert_eq!(
        typed(&client, &[Y, A]),
        [
            Key::Char('z'),
            Key::Char('Z'),
            Key::Char('A'),
            Key::Char('z')
        ]
    );
    assert_eq!(client.received.shell.keymaps.len(), 1);
    assert!(matches!(
        keyboard.press(KeyCode::UNKNOWN),
        Err(Error::UnknownKeyCode)
    ));
}

#[test]
fn physical_keyboard_releases_held_keys() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let _devices = client.devices();
    focused_window(&mut client);

    let seat = Seat::connect_to(harness.socket()).unwrap();
    let keyboard = seat.physical_keyboard().unwrap();
    keyboard.press(key(KEY_LEFTCTRL)).unwrap();
    keyboard.press(key(A)).unwrap();
    keyboard.press(key(A)).unwrap();
    client.until("held control and A", |recorded| {
        recorded.shell.keys.contains(&A) && recorded.shell.mods.depressed != 0
    });
    keyboard.release_all().unwrap();
    client.until("released held keys", |recorded| {
        recorded.shell.lifted.contains(&A)
            && recorded.shell.lifted.contains(&KEY_LEFTCTRL)
            && recorded.shell.mods.depressed == 0
    });

    keyboard.press(key(A)).unwrap();
    client.until("held A again", |recorded| {
        count(&recorded.shell.keys, A) == 2
    });
    drop(keyboard);
    client.until("released on drop", |recorded| {
        count(&recorded.shell.lifted, A) == 2
    });
    assert_eq!(count(&client.received.shell.keys, A), 2);
}

#[test]
fn text_keyboard_chords_press_and_release_their_modifier_keys() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let _devices = client.devices();
    focused_window(&mut client);

    let seat = Seat::connect_to(harness.socket()).unwrap();
    let keyboard = seat.text_keyboard().unwrap();
    keyboard
        .press(Chord::new(Modifiers::CONTROL, Key::Char('a')))
        .unwrap();
    client.until("released the chord", |recorded| {
        recorded.shell.lifted.contains(&KEY_LEFTCTRL) && recorded.shell.mods.depressed == 0
    });
    let shell = &client.received.shell;
    let order = |codes: &[u32], code: u32| {
        codes
            .iter()
            .position(|recorded| *recorded == code)
            .expect("the key was sent")
    };
    assert!(order(&shell.keys, KEY_LEFTCTRL) < order(&shell.keys, A));
    assert!(order(&shell.lifted, A) < order(&shell.lifted, KEY_LEFTCTRL));
}

#[test]
fn output_pointer_moves_absolute_and_relative() {
    let harness = Harness::full(EXTENT, &[]);
    let mut client = harness.client();
    let devices = client.devices();
    let _relative = client.relative_pointer(&devices.pointer);
    covering_layer(&mut client);

    let seat = Seat::connect_to(harness.socket()).unwrap();
    assert!(matches!(
        seat.pointer_on_output("nowhere"),
        Err(Error::OutputNotFound(_))
    ));
    let pointer = seat.pointer_on_output(OUTPUT).unwrap();
    pointer
        .move_to(AbsolutePosition::new(500, 250, 1000, 1000))
        .unwrap();
    client.until("pointer placed on output", |recorded| {
        recorded.shell.pointer_at == Some((160.0, 60.0))
    });
    pointer.move_by(12.0, -7.5).unwrap();
    client.until("pointer moved by delta", |recorded| {
        recorded.shell.pointer_at == Some((172.0, 52.5))
            && recorded.input.travelled.contains(&(12.0, -7.5))
    });
}

#[test]
fn pointer_scrolls_by_value120_and_continuously() {
    let harness = Harness::full(EXTENT, &[]);
    let mut client = harness.client();
    let _devices = client.devices();
    covering_layer(&mut client);

    let seat = Seat::connect_to(harness.socket()).unwrap();
    let pointer = seat.pointer().unwrap();
    pointer
        .move_to(AbsolutePosition::new(100, 100, EXTENT.width, EXTENT.height))
        .unwrap();
    pointer.scroll_value120(0, 60).unwrap();
    pointer.sync().unwrap();
    pointer.scroll_value120(0, 60).unwrap();
    pointer.sync().unwrap();
    pointer.scroll_continuous(-4.0, 30.5).unwrap();
    client.until("three scroll frames", |recorded| {
        recorded
            .shell
            .scrolled
            .iter()
            .filter(|scrolled| **scrolled == Scrolled::Frame)
            .count()
            == 3
    });

    assert_eq!(
        client.received.shell.scrolled,
        [
            Scrolled::Source(AxisSource::Wheel),
            Scrolled::Value(Axis::VerticalScroll, 7.5),
            Scrolled::Frame,
            Scrolled::Source(AxisSource::Wheel),
            Scrolled::Value120(Axis::VerticalScroll, 120),
            Scrolled::Value(Axis::VerticalScroll, 7.5),
            Scrolled::Frame,
            Scrolled::Source(AxisSource::Continuous),
            Scrolled::Value(Axis::HorizontalScroll, -4.0),
            Scrolled::Value(Axis::VerticalScroll, 30.5),
            Scrolled::Frame,
        ]
    );
}
