use std::time::Duration;

use crate::common::{Server, State};
use rikka_compositor_geometry::Point;
use rikka_compositor_seat::{ConstraintKind, ConstraintState, Focus, Motion, SeatEvent};
use rikka_wayland_client::wl_compositor;
use rikka_wayland_client::{
    zwp_confined_pointer_v1, zwp_locked_pointer_v1, zwp_pointer_constraints_v1,
};

fn hover(state: &mut State, id: Option<u32>, origin: (f64, f64), at: (f64, f64)) {
    let under = id.map(|id| Focus::new(state.surface(id), Point::new(origin.0, origin.1)));
    state.seat_mut().motion(Motion {
        location: Point::new(at.0, at.1),
        under,
        time: Duration::from_millis(2),
    });
    state.seat_mut().frame();
}

#[test]
fn oneshot_lock_ends_on_leave() {
    let server = Server::start();
    let mut client = server.connect();
    let seat = client.seat(0);
    let pointer = seat.get_pointer();
    let target = client.mapped(40, 40);
    let constraints: zwp_pointer_constraints_v1::ZwpPointerConstraintsV1 = client.global(1);
    let lock = constraints.lock_pointer(
        &target.surface,
        &pointer,
        None,
        zwp_pointer_constraints_v1::Lifetime::Oneshot,
    );
    client.roundtrip();
    assert!(client.received.locked.is_empty());

    let id = target.id;
    let position = server.call(move |state| {
        hover(state, Some(id), (100.0, 100.0), (110.0, 110.0));
        state.seat_mut().constrain(Point::new(300.0, 300.0))
    });
    assert_eq!(position, Point::new(110.0, 110.0));
    client.roundtrip();
    assert!(matches!(
        client.received.locked.as_slice(),
        [zwp_locked_pointer_v1::Event::Locked]
    ));

    lock.set_cursor_position_hint(4.0, 5.0);
    target.surface.commit();
    client.roundtrip();
    let events = server.call(move |state| {
        hover(state, None, (0.0, 0.0), (110.0, 110.0));
        hover(state, Some(id), (100.0, 100.0), (110.0, 110.0));
        let seat_mut = state.seat_mut;
        let active = state.seats.get(seat_mut).unwrap().pointer().constraint();
        let hinted = state.events.iter().any(|event| {
            matches!(
                event,
                SeatEvent::CursorHint { location, .. } if *location == Point::new(104.0, 105.0)
            )
        });
        let released = state.events.iter().any(|event| {
            matches!(
                event,
                SeatEvent::Constraint {
                    state: ConstraintState::Released,
                    ..
                }
            )
        });
        (active, hinted, released)
    });
    client.roundtrip();
    assert!(matches!(
        client.received.locked.as_slice(),
        [
            zwp_locked_pointer_v1::Event::Locked,
            zwp_locked_pointer_v1::Event::Unlocked
        ]
    ));
    assert_eq!(events, (None, true, true));

    let _second = constraints.lock_pointer(
        &target.surface,
        &pointer,
        None,
        zwp_pointer_constraints_v1::Lifetime::Oneshot,
    );
    assert!(client.fails());
}

#[test]
fn confinement_clamps_motion_to_region() {
    let server = Server::start();
    let mut client = server.connect();
    let seat = client.seat(0);
    let pointer = seat.get_pointer();
    let target = client.mapped(40, 40);
    let compositor: wl_compositor::WlCompositor = client.global(6);
    let region = compositor.create_region();
    region.add(10, 10, 10, 10);
    let constraints: zwp_pointer_constraints_v1::ZwpPointerConstraintsV1 = client.global(1);
    let _confine = constraints.confine_pointer(
        &target.surface,
        &pointer,
        Some(&region),
        zwp_pointer_constraints_v1::Lifetime::Persistent,
    );
    client.roundtrip();

    let id = target.id;
    let outside = server.call(move |state| {
        hover(state, Some(id), (100.0, 100.0), (102.0, 102.0));
        let seat_mut = state.seat_mut;
        state.seats.get(seat_mut).unwrap().pointer().constraint()
    });
    assert_eq!(outside, None);

    let clamped = server.call(move |state| {
        hover(state, Some(id), (100.0, 100.0), (115.0, 115.0));
        let seat_mut = state.seat_mut;
        let active = state.seats.get(seat_mut).unwrap().pointer().constraint();
        (
            active.map(|(_, kind)| kind),
            state.seat_mut().constrain(Point::new(300.0, 112.0)),
        )
    });
    assert_eq!(clamped.0, Some(ConstraintKind::Confine));
    assert!(clamped.1.x < 120.0 && clamped.1.x > 119.9);
    assert_eq!(clamped.1.y, 112.0);
    client.roundtrip();
    assert!(matches!(
        client.received.confined.as_slice(),
        [zwp_confined_pointer_v1::Event::Confined]
    ));

    let again = server.call(move |state| {
        hover(state, None, (0.0, 0.0), (0.0, 0.0));
        hover(state, Some(id), (100.0, 100.0), (115.0, 115.0));
        let seat_mut = state.seat_mut;
        state
            .seats
            .get(seat_mut)
            .unwrap()
            .pointer()
            .constraint()
            .is_some()
    });
    assert!(again);
    client.roundtrip();
    assert_eq!(client.received.confined.len(), 3);
}
