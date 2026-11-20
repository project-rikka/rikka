use std::time::Duration;

use rikka_compositor_geometry::Point as ServerPoint;
use rikka_compositor_seat::{Axis, AxisMotion, Focus, Motion};
use rikka_compositor_types::AxisSource;
use rikka_input::{DeviceEvent, ScrollSource};
use rikka_seat::SeatEvent;
use rikka_types::{Point, SCROLL_LINE, ScrollDelta};

use crate::support::{Client, Server};

const DETENT: f64 = 15.0;
const VALUE120_PER_DETENT: i32 = 120;

fn wheel_towards_content_end(server: &Server, client: &mut Client, inverted: bool) -> ScrollDelta {
    let id = client.surface_id();
    server.call(move |state| {
        let surface = state.surface(id);
        let mut seat = state.seat();
        seat.motion(Motion {
            location: ServerPoint::new(10.0, 20.0),
            under: Some(Focus::new(surface, ServerPoint::new(0.0, 0.0))),
            time: Duration::from_millis(1),
        });
        seat.frame();
        let mut axis = Axis::new(Duration::from_millis(2));
        axis.source = Some(AxisSource::Wheel);
        axis.vertical = AxisMotion {
            value: DETENT,
            v120: Some(VALUE120_PER_DETENT),
            stopped: false,
            inverted,
        };
        seat.axis(axis);
        seat.frame();
    });
    client
        .take()
        .into_iter()
        .find_map(|event| match event {
            SeatEvent::Input {
                event:
                    DeviceEvent::Scroll {
                        delta,
                        source: ScrollSource::Wheel,
                        ..
                    },
                ..
            } => Some(delta),
            _ => None,
        })
        .expect("the wheel scrolls")
}

#[test]
fn wheel_scroll_advances_the_offset_and_reports_which_way_the_wheel_turned() {
    for (inverted, up) in [(false, -1.0), (true, 1.0)] {
        let server = Server::start();
        let mut client = Client::connect(&server);
        client.take();

        let delta = wheel_towards_content_end(&server, &mut client, inverted);

        assert_eq!(delta.offset(), Point::new(0.0, SCROLL_LINE), "{inverted}");
        assert_eq!(delta.is_natural(), inverted);
        assert_eq!(delta.up(), up, "{inverted}");
    }
}
