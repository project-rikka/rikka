use rikka_compositor_types::Press;

use crate::common::Server;
use crate::common::client::keymap_file;
use crate::common::{A, KEYMAP, Q};
use rikka_compositor_geometry::{Point, Rect, Size};
use rikka_compositor_seat::{Caret, KeySource, SeatEvent};
use rikka_wayland_client::wl_keyboard::KeyState;
use rikka_wayland_client::zwp_virtual_keyboard_manager_v1;
use rikka_wayland_client::{zwp_input_method_manager_v2, zwp_input_method_v2};
use rikka_wayland_client::{zwp_text_input_manager_v3, zwp_text_input_v3};
use rustix::fd::AsFd;

#[test]
fn text_input_exchanges_with_input_method() {
    let server = Server::start();
    let mut app = server.connect();
    let mut method = server.connect();

    let app_seat = app.seat(0);
    let _keyboard = app_seat.get_keyboard().with_key(0u32);
    let text_manager: zwp_text_input_manager_v3::ZwpTextInputManagerV3 = app.global(2);
    let text_input = text_manager.get_text_input(&app_seat);
    let field = app.mapped(200, 100);
    let field_id = field.id;
    server.call(move |state| state.focus_keyboard(field_id));
    app.roundtrip();
    assert!(app.received.text_input.is_empty());

    let method_seat = method.seat(0);
    let manager: zwp_input_method_manager_v2::ZwpInputMethodManagerV2 = method.global(1);
    let input_method = manager.get_input_method(&method_seat);
    let _grab = input_method.grab_keyboard();
    method.roundtrip();
    app.roundtrip();
    assert!(matches!(
        app.received.text_input.as_slice(),
        [zwp_text_input_v3::Event::Enter { .. }]
    ));
    assert_eq!(method.received.keymaps.len(), 1);

    text_input.enable();
    text_input.set_surrounding_text("かな", 3, 3);
    text_input.set_cursor_rectangle(12, 30, 2, 18);
    text_input.commit();
    app.roundtrip();
    method.roundtrip();
    assert!(matches!(
        method.received.input_method.as_slice(),
        [
            zwp_input_method_v2::Event::Activate,
            zwp_input_method_v2::Event::SurroundingText {
                cursor: 3,
                anchor: 3,
                ..
            },
            zwp_input_method_v2::Event::Done,
        ]
    ));
    let (caret, composing) = server.call(move |state| {
        let surface = state.surface(field_id);
        let seat_mut = state.seat_mut;
        let caret = state.events.iter().rev().find_map(|event| match event {
            SeatEvent::Caret { caret, .. } => Some(*caret),
            _ => None,
        });
        (
            caret.flatten()
                == Some(Caret {
                    surface,
                    rect: Rect::new(Point::new(12, 30), Size::new(2, 18)),
                }),
            state.seats.get(seat_mut).unwrap().composing() == Some(surface),
        )
    });
    assert!(caret && composing);

    server.call(|state| {
        state.type_key(A, Press::Pressed);
        state.type_key(A, Press::Released);
    });
    method.roundtrip();
    app.roundtrip();
    assert_eq!(
        method.received.grabbed_keys(),
        vec![(A, KeyState::Pressed), (A, KeyState::Released)]
    );
    assert!(app.received.keys().is_empty());

    input_method.set_preedit_string("日", 0, 3);
    input_method.commit_string("日本");
    input_method.commit(1);
    method.roundtrip();
    app.roundtrip();
    let tail: Vec<&zwp_text_input_v3::Event> = app.received.text_input.iter().skip(1).collect();
    assert!(matches!(
        tail.as_slice(),
        [
            zwp_text_input_v3::Event::PreeditString { text: Some(preedit), .. },
            zwp_text_input_v3::Event::CommitString { text: Some(committed) },
            zwp_text_input_v3::Event::Done { serial: 1 },
        ] if preedit == "日" && committed == "日本"
    ));

    text_input.disable();
    text_input.commit();
    app.roundtrip();
    method.roundtrip();
    assert!(matches!(
        &method.received.input_method[3..],
        [
            zwp_input_method_v2::Event::Deactivate,
            zwp_input_method_v2::Event::Done
        ]
    ));
    server.expect("Caret { seat: SeatId");
    let cleared = server.call(|state| {
        matches!(
            state
                .events
                .iter()
                .rev()
                .find(|event| matches!(event, SeatEvent::Caret { .. })),
            Some(SeatEvent::Caret { caret: None, .. })
        )
    });
    assert!(cleared);
}

#[test]
fn input_method_keys_bypass_grab() {
    let server = Server::start();
    let mut app = server.connect();
    let mut method = server.connect();
    let mut robot = server.connect();

    let app_seat = app.seat(0);
    let _keyboard = app_seat.get_keyboard().with_key(0u32);
    let field = app.mapped(50, 50);
    let field_id = field.id;
    server.call(move |state| {
        state.focus_keyboard(field_id);
        state.bindings.push(Q);
    });

    let method_seat = method.seat(0);
    let manager: zwp_input_method_manager_v2::ZwpInputMethodManagerV2 = method.global(1);
    let input_method = manager.get_input_method(&method_seat);
    let _grab = input_method.grab_keyboard();
    let boards: zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1 = method.global(1);
    let own = boards.create_virtual_keyboard(&method_seat);
    method.roundtrip();
    let forwarded = method.received.keymaps[0].clone();
    let (fd, size) = keymap_file(&forwarded);
    own.keymap(1, fd.as_fd(), size);
    own.send_key(1, A, 1);
    own.send_key(2, A, 0);
    method.roundtrip();
    app.roundtrip();
    assert_eq!(
        app.received.keys(),
        vec![(A, KeyState::Pressed), (A, KeyState::Released)]
    );
    assert!(method.received.grabbed_keys().is_empty());
    assert_eq!(app.received.keymaps.len(), 1);

    let robot_seat = robot.seat(0);
    let robot_boards: zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1 =
        robot.global(1);
    let virtual_keyboard = robot_boards.create_virtual_keyboard(&robot_seat);
    let (fd, size) = keymap_file(KEYMAP);
    virtual_keyboard.keymap(1, fd.as_fd(), size);
    virtual_keyboard.send_key(3, Q, 1);
    virtual_keyboard.send_key(4, Q, 0);
    virtual_keyboard.send_key(5, A, 1);
    robot.roundtrip();
    method.roundtrip();
    app.roundtrip();
    assert_eq!(method.received.grabbed_keys(), vec![(A, KeyState::Pressed)]);
    assert_eq!(app.received.keys().len(), 2);

    let sources = server.call(|state| {
        state
            .strokes
            .iter()
            .map(|keystroke| keystroke.source)
            .collect::<Vec<_>>()
    });
    assert!(matches!(sources[0], KeySource::InputMethod(_)));
    assert!(matches!(sources[2], KeySource::Virtual(_)));
    server.expect("KeyboardGrab { seat: SeatId");
}

const PRIVILEGED: [&str; 3] = [
    "zwp_input_method_manager_v2",
    "zwp_virtual_keyboard_manager_v1",
    "zwlr_virtual_pointer_manager_v1",
];

const PUBLIC: [&str; 10] = [
    "zwp_relative_pointer_manager_v1",
    "zwp_pointer_constraints_v1",
    "zwp_pointer_gestures_v1",
    "wp_cursor_shape_manager_v1",
    "wp_pointer_warp_v1",
    "zwp_keyboard_shortcuts_inhibit_manager_v1",
    "zwp_tablet_manager_v2",
    "zwp_text_input_manager_v3",
    "zwp_idle_inhibit_manager_v1",
    "ext_idle_notifier_v1",
];

#[test]
fn virtual_input_is_privileged() {
    let server = Server::start();
    let trusted = server.connect();
    let sandboxed = server.connect_unprivileged();
    for interface in PRIVILEGED {
        assert!(trusted.advertises(interface), "{interface}");
        assert!(!sandboxed.advertises(interface), "{interface}");
    }
    for interface in PUBLIC {
        assert!(trusted.advertises(interface), "{interface}");
        assert!(sandboxed.advertises(interface), "{interface}");
    }
    assert!(!trusted.advertises("zwp_xwayland_keyboard_grab_manager_v1"));
}
