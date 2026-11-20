use std::time::Duration;

use rikka_compositor_types::Press;

use crate::common::{Server, State};
use rikka_compositor_geometry::Point;
use rikka_compositor_seat::{
    Button, CursorImage, CursorShape, Focus, GrabFocus, Motion, PointerGrab, PointerHandle,
    SeatEvent,
};
use rikka_wayland_client::wl_pointer;
use rikka_wayland_client::{wp_cursor_shape_device_v1, wp_cursor_shape_manager_v1};

const LEFT: u32 = 0x110;

fn entered(received: &crate::common::Received) -> Vec<(u32, u32)> {
    received
        .pointer
        .iter()
        .filter_map(|event| match event {
            wl_pointer::Event::Enter {
                serial, surface, ..
            } => Some((*serial, surface.id())),
            _ => None,
        })
        .collect()
}

fn left(received: &crate::common::Received) -> Vec<u32> {
    received
        .pointer
        .iter()
        .filter_map(|event| match event {
            wl_pointer::Event::Leave { surface, .. } => Some(surface.id()),
            _ => None,
        })
        .collect()
}

fn motion_to(state: &mut State, id: Option<(u32, (f64, f64))>, at: (f64, f64)) {
    let under =
        id.map(|(id, origin)| Focus::new(state.surface(id), Point::new(origin.0, origin.1)));
    state.seat_mut().motion(Motion {
        location: Point::new(at.0, at.1),
        under,
        time: Duration::from_millis(3),
    });
    state.seat_mut().frame();
}

fn click(state: &mut State, press: Press) -> u32 {
    let serial = state.seat_mut().button(Button {
        code: LEFT,
        press,
        time: Duration::from_millis(4),
    });
    state.seat_mut().frame();
    serial
}

#[test]
fn implicit_grab_keeps_focus() {
    let server = Server::start();
    let mut client = server.connect();
    let seat = client.seat(0);
    let _pointer = seat.get_pointer();
    let first = client.mapped(50, 50);
    let second = client.mapped(50, 50);
    let (a, b) = (first.id, second.id);

    server.call(move |state| {
        motion_to(state, Some((a, (0.0, 0.0))), (10.0, 10.0));
        click(state, Press::Pressed);
        motion_to(state, Some((b, (100.0, 0.0))), (110.0, 10.0));
    });
    client.roundtrip();
    assert_eq!(
        entered(&client.received)
            .iter()
            .map(|(_, id)| *id)
            .collect::<Vec<_>>(),
        vec![a]
    );
    assert!(client.received.pointer.iter().any(|event| matches!(
        event,
        wl_pointer::Event::Motion { surface_x, .. } if *surface_x == 110.0
    )));

    server.call(move |state| {
        click(state, Press::Released);
    });
    client.roundtrip();
    assert_eq!(left(&client.received), vec![a]);
    assert_eq!(
        entered(&client.received)
            .iter()
            .map(|(_, id)| *id)
            .collect::<Vec<_>>(),
        vec![a, b]
    );
    assert!(
        client
            .received
            .pointer
            .iter()
            .any(|event| matches!(event, wl_pointer::Event::Frame))
    );
}

#[test]
fn focus_changes_outside_input_end_their_frame() {
    type Change = fn(&mut State, u32);
    let cases: [(&str, Change); 3] = [
        ("warp onto a surface", |state, id| {
            let under = Some(Focus::new(state.surface(id), Point::new(0.0, 0.0)));
            state.seat_mut().warp(Motion {
                location: Point::new(5.0, 5.0),
                under,
                time: Duration::from_millis(3),
            });
        }),
        ("grab clearing focus", |state, id| {
            motion_to(state, Some((id, (0.0, 0.0))), (5.0, 5.0));
            state
                .seat_mut()
                .set_pointer_grab(Successor, GrabFocus::Clear);
        }),
        ("grab ending", |state, id| {
            motion_to(state, Some((id, (0.0, 0.0))), (5.0, 5.0));
            state
                .seat_mut()
                .set_pointer_grab(Successor, GrabFocus::Clear);
            state.seat_mut().end_pointer_grab();
        }),
    ];
    for (name, change) in cases {
        let server = Server::start();
        let mut client = server.connect();
        let seat = client.seat(0);
        let _pointer = seat.get_pointer();
        let id = client.mapped(50, 50).id;
        server.call(move |state| change(state, id));
        client.roundtrip();
        assert!(
            matches!(
                client.received.pointer.last(),
                Some(wl_pointer::Event::Frame)
            ),
            "{name}: {:?}",
            client.received.pointer
        );
    }
}

#[test]
fn stale_serial_rejects_cursor() {
    let server = Server::start();
    let mut client = server.connect();
    let seat = client.seat(0);
    let pointer = seat.get_pointer();
    let target = client.mapped(50, 50);
    let id = target.id;
    server.call(move |state| motion_to(state, Some((id, (0.0, 0.0))), (5.0, 5.0)));
    client.roundtrip();
    let (serial, _) = entered(&client.received)[0];

    let manager: wp_cursor_shape_manager_v1::WpCursorShapeManagerV1 = client.global(2);
    let device = manager.get_pointer(&pointer);
    device.set_shape(
        serial.wrapping_sub(1),
        wp_cursor_shape_device_v1::Shape::Wait,
    );
    device.set_shape(serial, wp_cursor_shape_device_v1::Shape::Grabbing);
    client.roundtrip();
    let shapes: Vec<CursorImage> = server.call(|state| {
        state
            .events
            .iter()
            .filter_map(|event| match event {
                SeatEvent::Cursor { image, .. } => Some(*image),
                _ => None,
            })
            .collect()
    });
    assert_eq!(
        shapes.last(),
        Some(&CursorImage::Shape(CursorShape::Grabbing))
    );
    assert!(!shapes.contains(&CursorImage::Shape(CursorShape::Wait)));
    assert_eq!(CursorShape::Grabbing.name(), "grabbing");
}

#[test]
fn scroll_sends_source_value120_and_stop() {
    let server = Server::start();
    let mut client = server.connect();
    let seat = client.seat(0);
    let _pointer = seat.get_pointer();
    let target = client.mapped(50, 50);
    let id = target.id;
    server.call(move |state| {
        motion_to(state, Some((id, (0.0, 0.0))), (5.0, 5.0));
        let mut axis = rikka_compositor_seat::Axis::new(Duration::from_millis(9));
        axis.source = Some(rikka_compositor_types::AxisSource::Wheel);
        axis.vertical = rikka_compositor_seat::AxisMotion {
            value: 15.0,
            v120: Some(120),
            stopped: false,
            inverted: true,
        };
        axis.horizontal.stopped = true;
        state.seat_mut().axis(axis);
        state.seat_mut().frame();
    });
    client.roundtrip();
    let tail: Vec<&wl_pointer::Event> = client
        .received
        .pointer
        .iter()
        .skip_while(|event| !matches!(event, wl_pointer::Event::AxisSource { .. }))
        .collect();
    assert!(matches!(tail[0], wl_pointer::Event::AxisSource { .. }));
    assert!(matches!(tail[1], wl_pointer::Event::AxisStop { .. }));
    assert!(matches!(
        tail[2],
        wl_pointer::Event::AxisRelativeDirection { .. }
    ));
    assert!(matches!(
        tail[3],
        wl_pointer::Event::AxisValue120 { value120: 120, .. }
    ));
    assert!(matches!(tail[4], wl_pointer::Event::Axis { value, .. } if *value == 15.0));
    assert!(matches!(tail[5], wl_pointer::Event::Frame));
}

#[derive(Debug)]
struct Handover;

impl PointerGrab<State> for Handover {
    fn motion(&mut self, pointer: &mut PointerHandle<'_, State>, _motion: &Motion) {
        pointer.state().note("handover saw motion");
        let seat = pointer.seat();
        rikka_compositor_seat::seat(pointer.state(), seat)
            .set_pointer_grab(Successor, GrabFocus::Keep);
    }

    fn ended(&mut self, state: &mut State) {
        state.note("handover ended");
    }
}

#[derive(Debug)]
struct Successor;

impl PointerGrab<State> for Successor {
    fn motion(&mut self, pointer: &mut PointerHandle<'_, State>, motion: &Motion) {
        pointer
            .state()
            .note(format!("successor saw {:?}", motion.location));
    }

    fn ended(&mut self, state: &mut State) {
        state.note("successor ended");
    }
}

#[test]
fn grab_replaced_during_dispatch_ends() {
    let server = Server::start();
    let _client = server.connect();
    let grabbed = server.call(|state| {
        state.seat_mut().set_pointer_grab(Handover, GrabFocus::Keep);
        motion_to(state, None, (1.0, 1.0));
        let seat_mut = state.seat_mut;
        let during = state.seats.get(seat_mut).unwrap().pointer().grabbed();
        motion_to(state, None, (2.0, 2.0));
        motion_to(state, None, (3.0, 3.0));
        state.seat_mut().end_pointer_grab();
        let after = state.seats.get(seat_mut).unwrap().pointer().grabbed();
        (during, after)
    });
    assert_eq!(grabbed, (true, false));
    server.expect("handover saw motion");
    server.expect("handover ended");
    server.expect("successor saw Point<Logical> { x: 2.0, y: 2.0 }");
    server.expect("successor saw Point<Logical> { x: 3.0, y: 3.0 }");
    server.expect("successor ended");
}
