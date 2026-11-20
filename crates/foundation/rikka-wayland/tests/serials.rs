use std::os::unix::net::UnixStream;

use rikka_wayland::{SerialKind, Serials};
use rikka_wayland_client::wl_seat::WlSeat;

fn two_seats() -> (UnixStream, WlSeat, WlSeat) {
    let (client, server) = UnixStream::pair().expect("socket pair");
    let (_connection, display) = rikka_wayland_client::from_socket(client).expect("connection");
    let registry = display.get_registry();
    let first: WlSeat = registry.bind(1, 1);
    let second: WlSeat = registry.bind(2, 1);
    (server, first, second)
}

#[test]
fn recorded_serials_follow_the_seat_every_holder_shares() {
    let (_server, first, second) = two_seats();
    let (serials, recorder) = Serials::recorded();
    let held = serials.clone();
    recorder.follow(Some(first.clone()));

    recorder.record(&first, SerialKind::ButtonPressed, 5);
    recorder.record(&second, SerialKind::KeyPressed, 6);
    assert_eq!(
        held.latest(&SerialKind::ALL).map(|latest| latest.serial),
        Some(5)
    );

    recorder.follow(Some(second.clone()));
    assert_eq!(held.seat(), Some(second.clone()));
    assert_eq!(held.latest(&SerialKind::ALL), None);

    recorder.record(&first, SerialKind::ButtonPressed, 7);
    recorder.record(&second, SerialKind::KeyPressed, 8);
    let latest = held
        .latest(&SerialKind::ALL)
        .expect("a serial of the followed seat");
    assert_eq!((latest.serial, latest.kind), (8, SerialKind::KeyPressed));

    recorder.follow(None);
    assert_eq!(held.seat(), None);
    assert_eq!(held.latest(&SerialKind::ALL), None);
}
