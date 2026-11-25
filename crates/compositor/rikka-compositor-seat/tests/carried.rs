use std::time::Duration;

use rikka_compositor_geometry::Point;
use rikka_compositor_seat::{
    Button, CursorImage, Focus, InputDevice, Key, KeySource, KeyboardFocus, Motion, SeatEvent,
    SeatId, SeatMut,
};
use rikka_compositor_types::Press;
use rikka_wayland_client::{wl_keyboard, wl_pointer};

use crate::common::{A, Q, Received, Server, State};

const LEFT: u32 = 0x110;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Seat {
    Carrier,
    Carried,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Seen {
    Enter(u32),
    Leave(u32),
    Moved,
    Pressed(u32),
    Released(u32),
}

fn seat(state: &mut State, seat: Seat) -> SeatMut<'_, State> {
    match seat {
        Seat::Carrier => state.seat_mut(),
        Seat::Carried => state.carried_mut(),
    }
}

fn seat_id(state: &State, seat: Seat) -> SeatId {
    match seat {
        Seat::Carrier => state.seat_mut,
        Seat::Carried => state.carried.unwrap(),
    }
}

fn move_to(state: &mut State, which: Seat, id: Option<u32>, at: (f64, f64)) {
    let under = id.map(|id| Focus::new(state.surface(id), Point::new(f64::from(id) * 100.0, 0.0)));
    let mut pointer = seat(state, which);
    pointer.motion(Motion {
        location: Point::new(at.0, at.1),
        under,
        time: Duration::from_millis(3),
    });
    pointer.frame();
}

fn click(state: &mut State, which: Seat, press: Press) -> u32 {
    let mut pointer = seat(state, which);
    let serial = pointer.button(Button {
        code: LEFT,
        press,
        time: Duration::from_millis(4),
    });
    pointer.frame();
    serial
}

fn focus(state: &mut State, which: Seat, id: Option<u32>) {
    let focus = id.map(|id| KeyboardFocus::Surface(state.surface(id)));
    seat(state, which).set_keyboard_focus(focus);
}

fn type_key(state: &mut State, which: Seat, code: u32) {
    for press in [Press::Pressed, Press::Released] {
        seat(state, which).key(Key {
            code,
            press,
            time: Duration::from_millis(7),
            source: KeySource::Device(InputDevice(1)),
        });
    }
}

fn pointer_seen(received: &Received) -> Vec<Seen> {
    received
        .pointer
        .iter()
        .filter_map(|event| match event {
            wl_pointer::Event::Enter { surface, .. } => Some(Seen::Enter(surface.id())),
            wl_pointer::Event::Leave { surface, .. } => Some(Seen::Leave(surface.id())),
            wl_pointer::Event::Motion { .. } => Some(Seen::Moved),
            wl_pointer::Event::Button {
                button,
                state: Ok(wl_pointer::ButtonState::Pressed),
                ..
            } => Some(Seen::Pressed(*button)),
            wl_pointer::Event::Button {
                button,
                state: Ok(wl_pointer::ButtonState::Released),
                ..
            } => Some(Seen::Released(*button)),
            _ => None,
        })
        .collect()
}

fn keyboard_seen(received: &Received) -> Vec<Seen> {
    received
        .keyboard
        .iter()
        .filter_map(|(_, event)| match event {
            wl_keyboard::Event::Enter { surface, .. } => Some(Seen::Enter(surface.id())),
            wl_keyboard::Event::Leave { surface, .. } => Some(Seen::Leave(surface.id())),
            wl_keyboard::Event::Key {
                key,
                state: Ok(wl_keyboard::KeyState::Pressed),
                ..
            } => Some(Seen::Pressed(*key)),
            _ => None,
        })
        .collect()
}

fn pressed_serials(received: &Received) -> Vec<u32> {
    received
        .pointer
        .iter()
        .filter_map(|event| match event {
            wl_pointer::Event::Button {
                serial,
                state: Ok(wl_pointer::ButtonState::Pressed),
                ..
            } => Some(*serial),
            _ => None,
        })
        .collect()
}

#[test]
fn carried_seat_is_hidden_from_ordinary_clients() {
    let server = Server::with_carried();
    assert_eq!(server.connect().seats().len(), 1);
    assert_eq!(server.connect_agent().seats().len(), 2);
}

#[test]
fn latest_pointer_input_presents_its_seat() {
    let server = Server::with_carried();
    let mut client = server.connect();
    let seat = client.seat(0);
    let _pointer = seat.get_pointer();
    let first = client.mapped(50, 50);
    let second = client.mapped(50, 50);
    let (a, b) = (first.id, second.id);

    server.call(move |state| {
        move_to(state, Seat::Carrier, Some(a), (10.0, 10.0));
        move_to(state, Seat::Carried, Some(b), (110.0, 10.0));
        move_to(state, Seat::Carried, None, (400.0, 10.0));
        move_to(state, Seat::Carried, Some(b), (112.0, 10.0));
        move_to(state, Seat::Carrier, Some(a), (12.0, 10.0));
    });
    client.roundtrip();

    use Seen::{Enter, Leave, Moved};
    assert_eq!(
        pointer_seen(&client.received),
        [
            Enter(a),
            Moved,
            Leave(a),
            Enter(b),
            Moved,
            Leave(b),
            Enter(a),
            Leave(a),
            Enter(b),
            Moved,
            Leave(b),
            Enter(a),
            Moved,
        ]
    );
}

#[test]
fn held_button_keeps_presenting_seat() {
    let server = Server::with_carried();
    let mut client = server.connect();
    let seat = client.seat(0);
    let _pointer = seat.get_pointer();
    let first = client.mapped(50, 50);
    let second = client.mapped(50, 50);
    let (a, b) = (first.id, second.id);

    let (carrier, carried) = server.call(move |state| {
        move_to(state, Seat::Carrier, Some(a), (10.0, 10.0));
        click(state, Seat::Carrier, Press::Pressed);
        move_to(state, Seat::Carried, Some(b), (110.0, 10.0));
        click(state, Seat::Carried, Press::Pressed);
        click(state, Seat::Carried, Press::Released);
        click(state, Seat::Carrier, Press::Released);
        move_to(state, Seat::Carried, Some(b), (111.0, 10.0));
        click(state, Seat::Carried, Press::Pressed);
        (seat_id(state, Seat::Carrier), seat_id(state, Seat::Carried))
    });
    client.roundtrip();

    use Seen::{Enter, Leave, Moved, Pressed, Released};
    assert_eq!(
        pointer_seen(&client.received),
        [
            Enter(a),
            Moved,
            Pressed(LEFT),
            Released(LEFT),
            Leave(a),
            Enter(b),
            Moved,
            Pressed(LEFT),
        ]
    );
    let serials = pressed_serials(&client.received);
    let issuers = server.call(move |state| {
        serials
            .iter()
            .map(|serial| state.seats.issuer(carrier, *serial))
            .collect::<Vec<_>>()
    });
    assert_eq!(issuers, [Some(carrier), Some(carried)]);
}

#[test]
fn latest_key_input_presents_its_seat() {
    let server = Server::with_carried();
    let mut client = server.connect();
    let seat = client.seat(0);
    let _keyboard = seat.get_keyboard().with_key(0u32);
    let first = client.mapped(50, 50);
    let second = client.mapped(50, 50);
    let (a, b) = (first.id, second.id);

    server.call(move |state| {
        focus(state, Seat::Carrier, Some(a));
        focus(state, Seat::Carried, Some(b));
        type_key(state, Seat::Carrier, A);
        type_key(state, Seat::Carried, Q);
        focus(state, Seat::Carried, None);
    });
    client.roundtrip();

    use Seen::{Enter, Leave, Pressed};
    assert_eq!(
        keyboard_seen(&client.received),
        [
            Enter(a),
            Leave(a),
            Enter(b),
            Leave(b),
            Enter(a),
            Pressed(A),
            Leave(a),
            Enter(b),
            Pressed(Q),
            Leave(b),
            Enter(a),
        ]
    );
}

#[test]
fn carried_presentation_leaves_carrier_cursor() {
    let server = Server::with_carried();
    let mut client = server.connect();
    let seat = client.seat(0);
    let pointer = seat.get_pointer();
    let first = client.mapped(50, 50);
    let second = client.mapped(50, 50);
    let (a, b) = (first.id, second.id);

    server.call(move |state| {
        move_to(state, Seat::Carrier, Some(a), (10.0, 10.0));
        move_to(state, Seat::Carried, Some(b), (110.0, 10.0));
        state.events.clear();
    });
    client.roundtrip();
    let entered = client
        .received
        .pointer
        .iter()
        .filter_map(|event| match event {
            wl_pointer::Event::Enter { serial, .. } => Some(*serial),
            _ => None,
        })
        .next_back()
        .unwrap();
    pointer.set_cursor(entered, None, 0, 0);
    client.roundtrip();

    let (carrier, carried, cursors) = server.call(|state| {
        let cursors: Vec<(SeatId, CursorImage)> = state
            .events
            .iter()
            .filter_map(|event| match event {
                SeatEvent::Cursor { seat, image } => Some((*seat, *image)),
                _ => None,
            })
            .collect();
        (
            seat_id(state, Seat::Carrier),
            seat_id(state, Seat::Carried),
            cursors,
        )
    });
    assert_eq!(cursors, [(carried, CursorImage::Hidden)]);
    assert!(cursors.iter().all(|(seat, _)| *seat != carrier));
}
