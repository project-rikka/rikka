use rikka_compositor_types::Press;

use crate::common::client::keymap_file;
use crate::common::{A, KEYMAP, Q, SHIFT, Server, keymap};
use rikka_compositor_seat::{KeyboardFocus, Keymap, SeatEvent, SeatParts};
use rikka_wayland_client::wl_keyboard::{self, KeyState};
use rikka_wayland_client::zwp_virtual_keyboard_manager_v1;
use rikka_wayland_client::{
    zwp_keyboard_shortcuts_inhibit_manager_v1, zwp_keyboard_shortcuts_inhibitor_v1,
};
use rustix::fd::AsFd;

fn enters(received: &crate::common::Received) -> Vec<(u32, u32, Vec<u8>)> {
    received
        .keyboard
        .iter()
        .filter_map(|(_, event)| match event {
            wl_keyboard::Event::Enter {
                serial,
                surface,
                keys,
            } => Some((*serial, surface.id(), keys.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn keyboard_receives_keymap_keys_and_modifiers() {
    let server = Server::start();
    let mut client = server.connect();
    let seat = client.seat(0);
    let keyboard = seat.get_keyboard().with_key(0u32);
    let mapped = client.mapped(10, 10);
    assert!(keyboard.alive());
    assert_eq!(client.received.keymaps.len(), 1);
    assert!(client.received.keymaps[0].contains("xkb_keymap"));
    assert!(client.received.keyboard.iter().any(|(_, event)| matches!(
        event,
        wl_keyboard::Event::RepeatInfo {
            rate: 25,
            delay: 600
        }
    )));

    let id = mapped.id;
    let strokes = server.call(move |state| {
        state.focus_keyboard(id);
        state.type_key(SHIFT, Press::Pressed);
        state.type_key(A, Press::Pressed);
        state.type_key(A, Press::Released);
        state.type_key(SHIFT, Press::Released);
        state.strokes.clone()
    });
    client.roundtrip();

    assert_eq!(
        client.received.keys(),
        vec![
            (SHIFT, KeyState::Pressed),
            (A, KeyState::Pressed),
            (A, KeyState::Released),
            (SHIFT, KeyState::Released),
        ]
    );
    let depressed: Vec<u32> = client
        .received
        .keyboard
        .iter()
        .filter_map(|(_, event)| match event {
            wl_keyboard::Event::Modifiers { mods_depressed, .. } => Some(*mods_depressed),
            _ => None,
        })
        .collect();
    assert_eq!(depressed, vec![0, 1, 0]);
    assert_eq!(strokes[1].sym.raw(), u32::from(b'A'));
    assert_eq!(strokes[1].latin.map(|sym| sym.raw()), Some(u32::from(b'a')));
    assert!(strokes[1].modifiers.shift);
    assert!(!strokes[3].modifiers.shift);
}

#[test]
fn focus_change_leaves_then_enters_with_keys() {
    let server = Server::start();
    let mut client = server.connect();
    let seat = client.seat(0);
    let _keyboard = seat.get_keyboard().with_key(0u32);
    let first = client.mapped(10, 10);
    let second = client.mapped(10, 10);
    let (first_id, second_id) = (first.id, second.id);
    server.call(move |state| {
        state.focus_keyboard(first_id);
        state.type_key(Q, Press::Pressed);
        state.focus_keyboard(second_id);
    });
    client.roundtrip();

    let entered = enters(&client.received);
    assert_eq!(entered.len(), 2);
    assert_eq!(entered[0].1, first_id);
    assert_eq!(entered[1].1, second_id);
    assert_eq!(entered[1].2, Q.to_ne_bytes().to_vec());
    let left: Vec<u32> = client
        .received
        .keyboard
        .iter()
        .filter_map(|(_, event)| match event {
            wl_keyboard::Event::Leave { serial, .. } => Some(*serial),
            _ => None,
        })
        .collect();
    assert_eq!(left.len(), 1);
    assert!(entered[0].0 < left[0]);
    assert!(left[0] <= entered[1].0);

    server.call(|state| state.type_key(Q, Press::Released));
    client.roundtrip();
    assert_eq!(
        client.received.keys(),
        vec![(Q, KeyState::Pressed), (Q, KeyState::Released)]
    );
}

#[test]
fn filtered_keys_and_releases_skip_client() {
    let server = Server::start();
    let mut client = server.connect();
    let seat = client.seat(0);
    let _keyboard = seat.get_keyboard().with_key(0u32);
    let mapped = client.mapped(10, 10);
    let id = mapped.id;
    let received = server.call(move |state| {
        state.bindings.push(Q);
        state.focus_keyboard(id);
        state.type_key(Q, Press::Pressed);
        state.bindings.clear();
        state.type_key(Q, Press::Released);
        state.type_key(A, Press::Pressed);
        state.strokes.len()
    });
    client.roundtrip();
    assert_eq!(received, 3);
    assert_eq!(client.received.keys(), vec![(A, KeyState::Pressed)]);
}

#[test]
fn withheld_keyboard_reenters_with_keys() {
    let server = Server::start();
    let mut client = server.connect();
    let seat = client.seat(0);
    let _keyboard = seat.get_keyboard().with_key(0u32);
    let first = client.mapped(10, 10);
    let second = client.mapped(10, 10);
    let (first_id, second_id) = (first.id, second.id);
    server.call(move |state| {
        state.focus_keyboard(first_id);
        state.type_key(SHIFT, Press::Pressed);
        state.seat_mut().withhold_keyboard();
        state.type_key(A, Press::Pressed);
        state.type_key(A, Press::Released);
        state.type_key(SHIFT, Press::Released);
        state.type_key(Q, Press::Pressed);
        state.seat_mut().restore_keyboard();
        state.type_key(Q, Press::Released);
    });
    client.roundtrip();

    let entered = enters(&client.received);
    assert_eq!(
        entered
            .iter()
            .map(|(_, surface, _)| *surface)
            .collect::<Vec<_>>(),
        vec![first_id, first_id]
    );
    assert_eq!(entered[1].2, Q.to_ne_bytes().to_vec());
    assert_eq!(
        client.received.keys(),
        vec![(SHIFT, KeyState::Pressed), (Q, KeyState::Released)]
    );

    server.call(move |state| {
        state.seat_mut().withhold_keyboard();
        state.focus_keyboard(second_id);
        state.seat_mut().restore_keyboard();
    });
    client.roundtrip();
    let entered = enters(&client.received);
    assert_eq!(
        entered.last().map(|(_, surface, _)| *surface),
        Some(second_id)
    );
    assert_eq!(entered.len(), 3);
    let left = client
        .received
        .keyboard
        .iter()
        .filter(|(_, event)| matches!(event, wl_keyboard::Event::Leave { .. }))
        .count();
    assert_eq!(left, 2);
}

#[test]
fn shortcuts_inhibitor_passes_bound_keys() {
    let server = Server::start();
    let mut client = server.connect();
    let seat = client.seat(0);
    let _keyboard = seat.get_keyboard().with_key(0u32);
    let mapped = client.mapped(10, 10);
    let manager: zwp_keyboard_shortcuts_inhibit_manager_v1::ZwpKeyboardShortcutsInhibitManagerV1 =
        client.global(1);
    let _inhibitor = manager.inhibit_shortcuts(&mapped.surface, &seat);
    client.roundtrip();
    assert!(matches!(
        client.received.inhibitor.as_slice(),
        [zwp_keyboard_shortcuts_inhibitor_v1::Event::Active]
    ));

    let id = mapped.id;
    let inhibited = server.call(move |state| {
        state.bindings.push(Q);
        state.focus_keyboard(id);
        state.type_key(Q, Press::Pressed);
        let seat_mut = state.seat_mut;
        state
            .seats
            .get(seat_mut)
            .unwrap()
            .keyboard()
            .shortcuts_inhibited()
    });
    client.roundtrip();
    assert!(inhibited);
    assert_eq!(client.received.keys(), vec![(Q, KeyState::Pressed)]);

    let _again = manager.inhibit_shortcuts(&mapped.surface, &seat);
    assert!(client.fails());
}

#[test]
fn unfocused_keyboard_follows_seat_keymap() {
    let server = Server::start();
    let mut app = server.connect();
    let mut robot = server.connect();
    let mut remote = server.connect();
    let app_seat = app.seat(0);
    let _keyboard = app_seat.get_keyboard().with_key(0u32);
    let field = app.mapped(10, 10);
    let id = field.id;
    server.call(move |state| {
        state.focus_keyboard(id);
    });

    let robot_seat = robot.seat(0);
    let boards: zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1 = robot.global(1);
    let board = boards.create_virtual_keyboard(&robot_seat);
    let (fd, size) = keymap_file(&KEYMAP.replace("[ a, A ]", "[ b, B ]"));
    board.keymap(1, fd.as_fd(), size);
    board.send_key(1, A, 1);
    board.send_key(2, A, 0);
    robot.roundtrip();
    app.roundtrip();
    assert_eq!(app.received.keymaps.len(), 2);

    let remote_seat = remote.seat(0);
    let _remote_keyboard = remote_seat.get_keyboard().with_key(0u32);
    remote.roundtrip();
    let changed = KEYMAP.replace("[ q, Q ]", "[ w, W ]");
    let changed = server.call(move |state| {
        let changed = Keymap::from_text(&changed).unwrap();
        let text = changed.text().to_owned();
        state.seat_mut().set_keymap(changed);
        text
    });
    remote.roundtrip();
    assert_eq!(
        remote.received.keymaps,
        vec![keymap().text().to_owned(), changed]
    );
}

#[test]
fn parts_hold_focus_reports_until_flushed() {
    let server = Server::start();
    let mut client = server.connect();
    let seat = client.seat(0);
    let _keyboard = seat.get_keyboard().with_key(0u32);
    let mapped = client.mapped(10, 10);
    let id = mapped.id;
    let (held, flushed) = server.call(move |state| {
        let surface = state.surface(id);
        let seat = state.seat_mut;
        state.events.clear();
        SeatParts::new(&mut state.seats, &mut state.surfaces, &state.display)
            .into_seat(seat)
            .set_keyboard_focus(Some(KeyboardFocus::Surface(surface)));
        let held = state.events.len();
        rikka_compositor_seat::flush(state);
        let flushed = state.events.iter().any(|event| {
            matches!(
                event,
                SeatEvent::KeyboardFocus {
                    focus: Some(KeyboardFocus::Surface(focused)),
                    ..
                } if *focused == surface
            )
        });
        (held, flushed)
    });
    client.roundtrip();
    assert_eq!(held, 0);
    assert!(flushed);
    assert_eq!(enters(&client.received).len(), 1);
}
