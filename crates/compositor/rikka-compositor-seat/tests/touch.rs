use std::time::Duration;

use crate::common::Server;
use rikka_compositor_geometry::Point;
use rikka_compositor_seat::{Capabilities, Focus, TouchDown, TouchMotion, TouchShape, TouchUp};

use rikka_wayland_client::{wl_seat, wl_touch};

#[test]
fn touch_points_keep_down_surface() {
    let server = Server::start();
    let mut client = server.connect();
    let seat = client.seat(0);
    server.call(|state| {
        state
            .seat_mut()
            .set_capabilities(Capabilities::POINTER | Capabilities::KEYBOARD | Capabilities::TOUCH);
    });
    client.roundtrip();
    let capabilities: Vec<wl_seat::Capability> = client
        .received
        .seat
        .iter()
        .filter_map(|(_, event)| match event {
            wl_seat::Event::Capabilities { capabilities } => Some(*capabilities),
            _ => None,
        })
        .collect();
    assert!(
        capabilities
            .last()
            .unwrap()
            .contains(wl_seat::Capability::TOUCH)
    );

    let _touch = seat.get_touch();
    let target = client.mapped(30, 30);
    let id = target.id;
    server.call(move |state| {
        let surface = state.surface(id);
        state.seat_mut().touch_down(TouchDown {
            slot: 2,
            location: Point::new(15.0, 25.0),
            under: Some(Focus::new(surface, Point::new(10.0, 20.0))),
            time: Duration::from_millis(1),
        });
        state.seat_mut().touch_shape(TouchShape {
            slot: 2,
            major: 4.0,
            minor: 3.0,
            orientation: Some(90.0),
        });
        state.seat_mut().touch_frame();
        state.seat_mut().touch_motion(TouchMotion {
            slot: 2,
            location: Point::new(200.0, 200.0),
            under: None,
            time: Duration::from_millis(2),
        });
        state.seat_mut().touch_frame();
        state.seat_mut().touch_up(TouchUp {
            slot: 2,
            time: Duration::from_millis(3),
        });
        state.seat_mut().touch_frame();
        state.seat_mut().touch_down(TouchDown {
            slot: 5,
            location: Point::new(11.0, 21.0),
            under: Some(Focus::new(surface, Point::new(10.0, 20.0))),
            time: Duration::from_millis(4),
        });
        state.seat_mut().touch_cancel();
    });
    client.roundtrip();

    let events = &client.received.touch;
    assert!(matches!(
        &events[0],
        wl_touch::Event::Down { id: 2, x, y, surface, .. }
            if *x == 5.0 && *y == 5.0 && surface.id() == id
    ));
    assert!(matches!(events[1], wl_touch::Event::Shape { id: 2, .. }));
    assert!(matches!(
        events[2],
        wl_touch::Event::Orientation { id: 2, .. }
    ));
    assert!(matches!(events[3], wl_touch::Event::Frame));
    assert!(matches!(
        events[4],
        wl_touch::Event::Motion { id: 2, x, y, .. } if x == 190.0 && y == 180.0
    ));
    assert!(matches!(events[5], wl_touch::Event::Frame));
    assert!(matches!(events[6], wl_touch::Event::Up { id: 2, .. }));
    assert!(matches!(events[7], wl_touch::Event::Frame));
    assert!(matches!(events[8], wl_touch::Event::Down { id: 5, .. }));
    assert!(matches!(events[9], wl_touch::Event::Cancel));
    assert_eq!(events.len(), 10);
}
