use rikka_nested::{ContentPurpose, Cursor, Input};
use rikka_types::{CursorShape, KeyCode, Point, PointerButton, Rect, ScrollDelta, Size};
use rikka_ui::{ScrollPhase, ScrollSource};
use rikka_wayland_client::wp_cursor_shape_device_v1::Shape;
use rikka_wayland_client::zwp_text_input_v3::{ContentHint, ContentPurpose as WireContentPurpose};

use crate::support::{self, Client, roundtrip_until};

fn logged(lines: &[String], expected: &str) -> bool {
    lines.iter().any(|line| line == expected)
}

#[test]
fn pointer_input_reaches_surface_under_pointer() {
    let (nested, _events) = support::start();
    let view = nested.view("rikka-view-pointer");
    let mut client = Client::connect(nested.socket());
    client.follow(&view);
    let _pointer = client.pointer();
    let window = client.window(Some("rikka-view-pointer"));
    client.map(&window, 100, 80);
    roundtrip_until(&mut client, |_| view.frame().layers.len() == 1);
    let surface = window.surface.id();

    view.deliver(Input::PointerMoved {
        at: Point::new(12.0, 34.0),
    });
    view.deliver(Input::Button {
        at: Point::new(12.0, 34.0),
        button: PointerButton::Primary,
        pressed: true,
    });
    view.deliver(Input::Button {
        at: Point::new(12.0, 34.0),
        button: PointerButton::Primary,
        pressed: false,
    });
    view.deliver(Input::Scroll {
        at: Point::new(12.0, 34.0),
        delta: ScrollDelta::from_offset(Point::new(0.0, 40.0)),
        source: ScrollSource::Wheel,
    });
    view.deliver(Input::Scroll {
        at: Point::new(12.0, 34.0),
        delta: ScrollDelta::from_offset(Point::new(0.0, 7.0)),
        source: ScrollSource::Continuous(ScrollPhase::Moved),
    });
    view.deliver(Input::Scroll {
        at: Point::new(12.0, 34.0),
        delta: ScrollDelta::ZERO,
        source: ScrollSource::Continuous(ScrollPhase::Ended),
    });
    roundtrip_until(&mut client, |client| {
        logged(&client.received.pointer, "stop Ok(VerticalScroll)")
            || logged(&client.received.pointer, "stop Value(VerticalScroll)")
    });
    let log = &client.received.pointer;
    assert!(logged(log, &format!("enter {surface} 12 34")), "{log:?}");
    assert!(logged(log, "button 0x110 true"), "{log:?}");
    assert!(logged(log, "button 0x110 false"), "{log:?}");
    assert!(
        log.iter()
            .any(|line| line.contains("value120") && line.ends_with(" 120")),
        "{log:?}"
    );
    assert!(
        log.iter()
            .any(|line| line.starts_with("source") && line.contains("Wheel")),
        "{log:?}"
    );
    assert!(
        log.iter()
            .any(|line| line.starts_with("source") && line.contains("Finger")),
        "{log:?}"
    );
    assert!(
        log.iter()
            .any(|line| line.starts_with("axis") && line.ends_with(" 7")),
        "{log:?}"
    );
}

#[test]
fn focus_sends_keymap_repeat_and_keys() {
    let (nested, _events) = support::start();
    let view = nested.view("rikka-view-keys");
    let mut client = Client::connect(nested.socket());
    client.follow(&view);
    let _keyboard = client.keyboard();
    let window = client.window(Some("rikka-view-keys"));
    client.map(&window, 50, 50);
    roundtrip_until(&mut client, |_| view.look().attached);
    view.deliver(Input::Focus(true));
    view.deliver(Input::Key {
        code: KeyCode::from_evdev(30),
        pressed: true,
    });
    view.deliver(Input::Key {
        code: KeyCode::from_evdev(30),
        pressed: true,
    });
    view.deliver(Input::Key {
        code: KeyCode::from_evdev(30),
        pressed: false,
    });
    roundtrip_until(&mut client, |client| {
        logged(&client.received.keyboard, "key 30 false")
    });
    let log = &client.received.keyboard;
    let surface = window.surface.id();
    assert!(logged(log, "keymap"), "{log:?}");
    assert!(logged(log, "repeat 33 400"), "{log:?}");
    assert!(logged(log, &format!("enter {surface}")), "{log:?}");
    assert_eq!(
        log.iter().filter(|line| *line == "key 30 true").count(),
        1,
        "{log:?}"
    );

    view.deliver(Input::Focus(false));
    roundtrip_until(&mut client, |client| {
        logged(&client.received.keyboard, &format!("leave {surface}"))
    });
}

#[test]
fn text_input_reports_state_and_composes() {
    let (nested, _events) = support::start();
    let view = nested.view("rikka-view-text");
    view.resize(Size::new(200.0, 100.0), 1.0);
    let mut client = Client::connect(nested.socket());
    client.follow(&view);
    let _keyboard = client.keyboard();
    let text_input = client.text_input();
    let window = client.window(Some("rikka-view-text"));
    client.map(&window, 200, 100);
    roundtrip_until(&mut client, |_| view.frame().layers.len() == 1);
    view.deliver(Input::Focus(true));
    roundtrip_until(&mut client, |client| logged(&client.received.text, "enter"));

    text_input.enable();
    text_input.set_content_type(ContentHint::SPELLCHECK, WireContentPurpose::Email);
    text_input.set_surrounding_text("hello", 5, 5);
    text_input.set_cursor_rectangle(20, 30, 2, 16);
    text_input.commit();
    roundtrip_until(&mut client, |_| {
        view.look().text_input.is_some() && view.look().caret.is_some()
    });
    let look = view.look();
    let state = look.text_input.as_ref().unwrap();
    assert_eq!(state.purpose, ContentPurpose::Email);
    assert!(state.hints.spellcheck);
    assert_eq!(state.surrounding.as_ref().unwrap().text, "hello");
    assert_eq!(look.caret, Some(Rect::new(20.0, 30.0, 2.0, 16.0)));

    view.deliver(Input::Preedit {
        text: "wor".into(),
        cursor: Some(3..3),
    });
    view.deliver(Input::Commit("world".into()));
    roundtrip_until(&mut client, |client| {
        logged(&client.received.text, "commit world")
    });
    assert!(logged(&client.received.text, "preedit wor 3 3"));
    assert!(logged(&client.received.text, "done 1"));

    text_input.disable();
    text_input.commit();
    roundtrip_until(&mut client, |_| view.look().text_input.is_none());
}

#[test]
fn hovered_client_cursor_reaches_view() {
    let (nested, _events) = support::start();
    let view = nested.view("rikka-view-cursor");
    let mut client = Client::connect(nested.socket());
    client.follow(&view);
    let pointer = client.pointer();
    let window = client.window(Some("rikka-view-cursor"));
    client.map(&window, 50, 50);
    roundtrip_until(&mut client, |_| view.frame().layers.len() == 1);
    view.deliver(Input::PointerMoved {
        at: Point::new(5.0, 5.0),
    });
    roundtrip_until(&mut client, |client| client.received.pointer_serial != 0);
    let device = client.cursor_shapes.get_pointer(&pointer);
    device.set_shape(client.received.pointer_serial, Shape::Text);
    roundtrip_until(&mut client, |_| {
        matches!(view.look().cursor, Cursor::Shape(CursorShape::Text))
    });
}
