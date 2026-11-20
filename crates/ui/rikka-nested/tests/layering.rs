use rikka_nested::Input;
use rikka_types::{Point, PointerButton, Rect};

use crate::support::{self, Client, roundtrip_until};

#[test]
fn children_stack_above_toplevel() {
    let (nested, _events) = support::start();
    let view = nested.view("rikka-view-stack");
    let mut client = Client::connect(nested.socket());
    client.follow(&view);
    let pointer = client.pointer();
    let window = client.window(Some("rikka-view-stack"));
    let (child, _subsurface) = client.subsurface(&window.surface, 10, 20);
    let child_buffer = client.shm_buffer(30, 10, 0xff00_ff00);
    child.attach(Some(&child_buffer), 0, 0);
    child.commit();
    client.map(&window, 100, 80);
    roundtrip_until(&mut client, |_| view.frame().layers.len() == 2);

    let popup = client.popup(&window.xdg, (50, 40), (20, 15), None);
    roundtrip_until(&mut client, |client| !client.received.popups.is_empty());
    assert_eq!(client.received.popups[0], (50, 40, 20, 15));
    client.ack(&popup.xdg);
    let popup_buffer = client.shm_buffer(20, 15, 0xffff_0000);
    client.show(&popup.surface, &popup_buffer);
    roundtrip_until(&mut client, |_| view.frame().layers.len() == 3);

    let rects: Vec<Rect> = view.frame().layers.iter().map(|layer| layer.rect).collect();
    assert_eq!(
        rects,
        [
            Rect::new(0.0, 0.0, 100.0, 80.0),
            Rect::new(10.0, 20.0, 30.0, 10.0),
            Rect::new(50.0, 40.0, 20.0, 15.0),
        ]
    );
    let ids: Vec<_> = view.frame().layers.iter().map(|layer| layer.id).collect();
    child.commit();
    client.roundtrip();
    roundtrip_until(&mut client, |_| view.frame().layers.len() == 3);
    let remaining: Vec<_> = view.frame().layers.iter().map(|layer| layer.id).collect();
    assert_eq!(ids, remaining);

    for (at, surface) in [
        (Point::new(15.0, 25.0), child.id()),
        (Point::new(55.0, 45.0), popup.surface.id()),
        (Point::new(95.0, 5.0), window.surface.id()),
    ] {
        view.deliver(Input::PointerMoved { at });
        let local = (at.x
            - rects
                .iter()
                .rev()
                .find(|rect| rect.contains(at))
                .unwrap()
                .origin
                .x) as i32;
        roundtrip_until(&mut client, |client| {
            client
                .received
                .pointer
                .iter()
                .any(|line| line.starts_with(&format!("enter {surface} {local}")))
        });
    }
    drop(pointer);
}

#[test]
fn outside_press_dismisses_grabbing_popup() {
    let (nested, _events) = support::start();
    let view = nested.view("rikka-view-grab");
    let mut client = Client::connect(nested.socket());
    client.follow(&view);
    let _pointer = client.pointer();
    let _keyboard = client.keyboard();
    let window = client.window(Some("rikka-view-grab"));
    client.map(&window, 100, 80);
    roundtrip_until(&mut client, |_| view.frame().layers.len() == 1);
    view.deliver(Input::Focus(true));
    let at = Point::new(10.0, 10.0);
    view.deliver(Input::Button {
        at,
        button: PointerButton::Primary,
        pressed: true,
    });
    roundtrip_until(&mut client, |client| {
        client
            .received
            .pointer
            .iter()
            .any(|line| line == "button 0x110 true")
    });
    let serial = client.received.pointer_serial;
    let popup = client.popup(&window.xdg, (20, 20), (30, 30), Some(serial));
    roundtrip_until(&mut client, |client| !client.received.popups.is_empty());
    client.ack(&popup.xdg);
    let buffer = client.shm_buffer(30, 30, 0xff00_00ff);
    client.show(&popup.surface, &buffer);
    roundtrip_until(&mut client, |_| view.frame().layers.len() == 2);
    let popup_surface = popup.surface.id();
    roundtrip_until(&mut client, |client| {
        client
            .received
            .keyboard
            .iter()
            .any(|line| *line == format!("enter {popup_surface}"))
    });

    view.deliver(Input::Button {
        at,
        button: PointerButton::Primary,
        pressed: false,
    });
    let pressed = |client: &Client| {
        client
            .received
            .pointer
            .iter()
            .filter(|line| *line == "button 0x110 true")
            .count()
    };
    let before = pressed(&client);
    view.deliver(Input::Button {
        at: Point::new(90.0, 70.0),
        button: PointerButton::Primary,
        pressed: true,
    });
    roundtrip_until(&mut client, |client| client.received.dismissed);
    assert_eq!(pressed(&client), before);
}
