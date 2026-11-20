use rikka_compositor_types::Press;

use crate::common::{Server, State, Wayland};
use crate::common::{drain, mimes, pipe};
use rikka_compositor_geometry::Point;
use rikka_compositor_protocols::wl_data_device_manager::DndAction as Wire;
use rikka_compositor_seat::Focus;
use rikka_compositor_seat::{TouchDown, TouchMotion, TouchUp};
use rikka_compositor_selection::DragDevice;
use rikka_compositor_selection::SelectionEvent;
use rikka_compositor_selection::{
    CompositorDragEvent, ForeignDragEvent, ForeignOffer, drop_compositor_drag, foreign_drag_data,
    foreign_drag_status, foreign_drop_finished, move_compositor_drag, start_compositor_drag,
};
use rikka_compositor_server::ClientId;
use rikka_wayland_client::wl_data_device_manager::DndAction;
use rustix::fd::AsFd;
use std::time::Duration;

const TEXT: &str = "text/plain";

fn press_on(server: &Server, client: rikka_compositor_server::ClientId, window: u32) -> u32 {
    server.call(move |state| {
        state.move_to(Some((client, window, (0.0, 0.0))), (5.0, 5.0));
        state.press(Press::Pressed)
    })
}

fn release(server: &Server) {
    server.call(|state| {
        state.press(Press::Released);
    });
}

fn offer_in(lines: &[String]) -> u32 {
    lines
        .iter()
        .find_map(|line| line.strip_prefix("data_offer "))
        .expect("an offer was introduced")
        .parse()
        .unwrap()
}

fn dragging(state: &mut State) -> bool {
    let seat_mut = state.seat_mut;
    state.selections.drag(seat_mut).is_some()
}

#[test]
fn drag_negotiates_action_and_finishes() {
    let server = Server::start();
    let mut a = server.connect();
    let mut b = server.connect();
    let desk_a = a.participant();
    let desk_b = b.participant();
    let (client_a, window_a) = (a.client, desk_a.window.id);
    let (client_b, window_b) = (b.client, desk_b.window.id);

    let serial = press_on(&server, client_a, window_a);
    a.roundtrip();
    let source = a.data_source(&desk_a.manager, "dragged", &[TEXT]);
    source.set_actions(DndAction::COPY | DndAction::MOVE);
    desk_a
        .device
        .start_drag(Some(&source), &desk_a.window.surface, None, serial);
    a.roundtrip();
    let seat_mut = server.call(|state| state.seat_mut);
    assert!(
        server.call(move |state| state.has_event(&SelectionEvent::DragStarted { seat: seat_mut }))
    );
    let lines = a.received.take_device();
    let own = offer_in(&lines);
    assert_eq!(
        lines.last().unwrap(),
        &format!("enter {window_a} 5 5 Some({own})")
    );

    server
        .call(move |state| state.move_to(Some((client_b, window_b, (100.0, 0.0))), (110.0, 10.0)));
    a.roundtrip();
    b.roundtrip();
    assert_eq!(a.received.take_device(), ["leave"]);
    let lines = b.received.take_device();
    let offer_id = offer_in(&lines);
    assert_eq!(
        lines.last().unwrap(),
        &format!("enter {window_b} 10 10 Some({offer_id})")
    );
    assert_eq!(b.received.offered(offer_id), [TEXT]);
    assert_eq!(b.received.offer_log, ["source_actions 3"]);

    let offer = b.data_offer(offer_id);
    offer.set_actions(DndAction::COPY | DndAction::MOVE, DndAction::MOVE);
    offer.accept(0, Some(TEXT));
    b.roundtrip();
    a.roundtrip();
    assert_eq!(
        b.received.offer_log.last().map(String::as_str),
        Some("action 2")
    );
    assert!(
        a.received
            .data_source
            .ends_with(&["action 2".to_owned(), format!("target Some({TEXT:?})")])
    );

    server.call(|state| state.move_to(None, (0.0, 0.0)));
    server
        .call(move |state| state.move_to(Some((client_b, window_b, (100.0, 0.0))), (112.0, 12.0)));
    b.roundtrip();
    let lines = b.received.take_device();
    assert_eq!(lines[0], "leave");
    let offer_id = offer_in(&lines);
    let offer = b.data_offer(offer_id);
    offer.set_actions(DndAction::COPY | DndAction::MOVE, DndAction::MOVE);
    offer.accept(0, Some(TEXT));
    b.roundtrip();
    server
        .call(move |state| state.move_to(Some((client_b, window_b, (100.0, 0.0))), (113.0, 14.0)));
    b.roundtrip();
    assert_eq!(b.received.take_device(), ["motion 13 14"]);

    release(&server);
    a.roundtrip();
    b.roundtrip();
    assert_eq!(b.received.take_device(), ["drop", "leave"]);
    assert_eq!(
        a.received.data_source.last().map(String::as_str),
        Some("dnd_drop_performed")
    );
    assert!(server.call(move |state| {
        state.has_event(&SelectionEvent::DragEnded {
            seat: seat_mut,
            dropped: true,
        })
    }));
    assert!(!server.call(dragging));

    let (read, write) = pipe();
    offer.receive(TEXT, write.as_fd());
    drop(write);
    b.roundtrip();
    a.roundtrip();
    assert_eq!(drain(read), "dragged");
    offer.finish();
    b.roundtrip();
    a.roundtrip();
    assert_eq!(
        a.received.data_source.last().map(String::as_str),
        Some("dnd_finished")
    );
}

#[test]
fn unaccepted_drop_cancels_source() {
    let server = Server::start();
    let mut a = server.connect();
    let mut b = server.connect();
    let desk_a = a.participant();
    let desk_b = b.participant();
    let (client_a, window_a) = (a.client, desk_a.window.id);
    let (client_b, window_b) = (b.client, desk_b.window.id);
    let seat_mut = server.call(|state| state.seat_mut);

    let serial = press_on(&server, client_a, window_a);
    let unaccepted = a.data_source(&desk_a.manager, "", &[TEXT]);
    unaccepted.set_actions(DndAction::COPY);
    desk_a
        .device
        .start_drag(Some(&unaccepted), &desk_a.window.surface, None, serial);
    a.roundtrip();
    server
        .call(move |state| state.move_to(Some((client_b, window_b, (100.0, 0.0))), (110.0, 10.0)));
    release(&server);
    a.roundtrip();
    b.roundtrip();
    assert_eq!(
        b.received.take_device().last().map(String::as_str),
        Some("leave")
    );
    assert_eq!(
        a.received.data_source.last().map(String::as_str),
        Some("cancelled")
    );
    assert!(server.call(move |state| {
        state.has_event(&SelectionEvent::DragEnded {
            seat: seat_mut,
            dropped: false,
        })
    }));

    let serial = press_on(&server, client_a, window_a);
    let nowhere = a.data_source(&desk_a.manager, "", &[TEXT]);
    nowhere.set_actions(DndAction::COPY);
    desk_a
        .device
        .start_drag(Some(&nowhere), &desk_a.window.surface, None, serial);
    a.roundtrip();
    server.call(|state| state.move_to(None, (500.0, 500.0)));
    assert_eq!(
        server.call(|state| state.selections.drag(state.seat_mut).unwrap().target),
        None
    );
    release(&server);
    a.roundtrip();
    assert_eq!(
        a.received.data_source.last().map(String::as_str),
        Some("cancelled")
    );
    assert!(!server.call(dragging));
}

#[test]
fn drag_requires_implicit_grab_serial() {
    let server = Server::start();
    let mut a = server.connect();
    let desk = a.participant();
    let (client, window) = (a.client, desk.window.id);
    let serial = press_on(&server, client, window);
    let source = a.data_source(&desk.manager, "", &[TEXT]);
    desk.device.start_drag(
        Some(&source),
        &desk.window.surface,
        None,
        serial.wrapping_add(1),
    );
    a.roundtrip();
    assert_eq!(a.received.data_source, ["cancelled"]);
    assert!(!server.call(dragging));

    release(&server);
    let late = a.data_source(&desk.manager, "", &[TEXT]);
    desk.device
        .start_drag(Some(&late), &desk.window.surface, None, serial);
    a.roundtrip();
    assert_eq!(a.received.data_source, ["cancelled", "cancelled"]);
    assert!(!server.call(dragging));
}

#[test]
fn sourceless_drag_stays_in_client() {
    let server = Server::start();
    let mut a = server.connect();
    let mut b = server.connect();
    let desk_a = a.participant();
    let desk_b = b.participant();
    let (client_a, window_a) = (a.client, desk_a.window.id);
    let (client_b, window_b) = (b.client, desk_b.window.id);

    let serial = press_on(&server, client_a, window_a);
    desk_a
        .device
        .start_drag(None, &desk_a.window.surface, None, serial);
    a.roundtrip();
    assert_eq!(
        a.received.take_device(),
        [format!("enter {window_a} 5 5 None")]
    );
    server
        .call(move |state| state.move_to(Some((client_b, window_b, (100.0, 0.0))), (110.0, 10.0)));
    server.call(move |state| state.move_to(Some((client_a, window_a, (0.0, 0.0))), (7.0, 8.0)));
    release(&server);
    a.roundtrip();
    b.roundtrip();
    assert!(b.received.device.is_empty());
    assert_eq!(
        a.received.take_device(),
        [
            "leave".to_owned(),
            format!("enter {window_a} 7 8 None"),
            "drop".to_owned(),
            "leave".to_owned()
        ]
    );
}

fn foreign_offer(state: &State) -> Option<ForeignOffer> {
    state.events.iter().rev().find_map(|event| match event {
        SelectionEvent::ForeignDrag {
            offer,
            event: ForeignDragEvent::Entered { .. },
            ..
        } => Some(*offer),
        _ => None,
    })
}

fn has_foreign_event(state: &State, expected: ForeignDragEvent) -> bool {
    state.events.iter().any(
        |event| matches!(event, SelectionEvent::ForeignDrag { event, .. } if *event == expected),
    )
}

#[test]
fn host_bridges_drag_over_foreign_surface() {
    let server = Server::start();
    let mut a = server.connect();
    let mut x = server.connect();
    let desk = a.participant();
    let (_bridge, bridge_id) = x.surface();
    x.roundtrip();
    let (client_a, window_a) = (a.client, desk.window.id);
    let client_x = x.client;
    server.call(move |state| {
        let surface = state.surface(client_x, bridge_id);
        state.foreign.push(surface);
    });

    let serial = server.call(move |state| {
        state.move_to(Some((client_a, window_a, (0.0, 0.0))), (5.0, 5.0));
        state.press(Press::Pressed)
    });
    let source = a.data_source(&desk.manager, "carried", &[TEXT]);
    source.set_actions(DndAction::COPY | DndAction::MOVE);
    desk.device
        .start_drag(Some(&source), &desk.window.surface, None, serial);
    a.roundtrip();

    server
        .call(move |state| state.move_to(Some((client_x, bridge_id, (200.0, 0.0))), (210.0, 20.0)));
    let offer = server
        .call(|state| foreign_offer(state))
        .expect("the host heard of the foreign target");
    let offered = server.call(move |state| {
        let info = state.selections.foreign_offer(offer).unwrap();
        (info.mime_types.to_vec(), info.actions)
    });
    assert_eq!(
        offered,
        (vec![Box::<str>::from(TEXT)], Wire::COPY | Wire::MOVE)
    );

    server.call(move |state| foreign_drag_status(state, offer, true, Wire::COPY));
    a.roundtrip();
    assert!(
        a.received
            .data_source
            .ends_with(&[format!("target Some({TEXT:?})"), "action 1".to_owned()])
    );

    let (read, write) = pipe();
    server.call(move |state| foreign_drag_data(state, offer, TEXT.to_owned(), write).unwrap());
    a.roundtrip();
    assert_eq!(drain(read), "carried");

    server.call(|state| {
        state.press(Press::Released);
    });
    a.roundtrip();
    assert!(server.call(|state| has_foreign_event(state, ForeignDragEvent::Dropped)));
    assert_eq!(
        a.received.data_source.last().map(String::as_str),
        Some("dnd_drop_performed")
    );

    server.call(move |state| foreign_drop_finished(state, offer, true, Some(Wire::COPY)));
    a.roundtrip();
    assert_eq!(
        a.received.data_source.last().map(String::as_str),
        Some("dnd_finished")
    );
}

#[test]
fn client_accepts_compositor_drag() {
    let server = Server::start();
    let mut b = server.connect();
    let desk = b.participant();
    b.roundtrip();
    let (client_b, window_b) = (b.client, desk.window.id);
    let seat_mut = server.call(|state| state.seat_mut);

    let source = server
        .call(move |state| {
            start_compositor_drag(
                state,
                seat_mut,
                mimes(&[TEXT]),
                Wire::COPY | Wire::MOVE,
                Point::new(0.0, 0.0),
            )
        })
        .expect("nothing else is dragged");
    server.call(move |state| {
        let under = Focus::new(state.surface(client_b, window_b), Point::new(100.0, 0.0));
        move_compositor_drag(
            state,
            seat_mut,
            Point::new(110.0, 10.0),
            Some(under),
            Duration::ZERO,
        );
    });
    b.roundtrip();
    let lines = b.received.take_device();
    let offer_id = lines
        .iter()
        .find_map(|line| line.strip_prefix("data_offer "))
        .expect("the client was offered the drag")
        .parse()
        .unwrap();
    assert_eq!(b.received.offered(offer_id), [TEXT]);
    let offer = b.data_offer(offer_id);
    offer.set_actions(DndAction::COPY, DndAction::COPY);
    offer.accept(0, Some(TEXT));
    b.roundtrip();
    assert!(server.call(move |state| {
        state.has_event(&SelectionEvent::CompositorDrag {
            seat: seat_mut,
            source,
            event: CompositorDragEvent::Target(true),
        })
    }));

    server.call(move |state| drop_compositor_drag(state, seat_mut));
    b.roundtrip();
    assert_eq!(b.received.take_device(), ["drop", "leave"]);

    let (read, write) = pipe();
    offer.receive(TEXT, write.as_fd());
    drop(write);
    b.roundtrip();
    assert_eq!(drain(read), "carried text/plain");
    offer.finish();
    b.roundtrip();
    assert!(server.call(move |state| {
        state.has_event(&SelectionEvent::CompositorDrag {
            seat: seat_mut,
            source,
            event: CompositorDragEvent::Finished(None),
        })
    }));
    offer.destroy();
    b.roundtrip();
    assert!(
        !server.call(move |state| state.selections.compositor_drag_offers(source, TEXT)),
        "a finished drag lets its source go"
    );
}

const SLOT: u32 = 3;

type Under = Option<(ClientId, u32, (f64, f64))>;

fn focus(state: &State, under: Under) -> Option<Focus> {
    under.map(|(client, id, origin)| {
        Focus::new(state.surface(client, id), Point::new(origin.0, origin.1))
    })
}

fn touch_down(server: &Server, client: ClientId, window: u32) {
    server.call(move |state| {
        let under = focus(state, Some((client, window, (0.0, 0.0))));
        state.seat_mut().touch_down(TouchDown {
            slot: SLOT,
            location: Point::new(5.0, 5.0),
            under,
            time: Duration::from_millis(1),
        });
        state.seat_mut().touch_frame();
    });
}

fn touch_move(server: &Server, slot: u32, under: Under, at: (f64, f64)) {
    server.call(move |state| {
        let under = focus(state, under);
        state.seat_mut().touch_motion(TouchMotion {
            slot,
            location: Point::new(at.0, at.1),
            under,
            time: Duration::from_millis(2),
        });
        state.seat_mut().touch_frame();
    });
}

fn start_touch_drag(
    server: &Server,
    a: &mut Wayland,
    desk: &crate::common::client::Participant,
) -> rikka_wayland_client::wl_data_source::WlDataSource {
    touch_down(server, a.client, desk.window.id);
    a.roundtrip();
    let serial = a.received.touch_down_serial();
    let source = a.data_source(&desk.manager, "touched", &[TEXT]);
    source.set_actions(DndAction::COPY);
    desk.device
        .start_drag(Some(&source), &desk.window.surface, None, serial);
    a.roundtrip();
    source
}

#[test]
fn touch_drag_follows_point_and_drops() {
    let server = Server::start();
    let mut a = server.connect();
    let mut b = server.connect();
    let desk_a = a.participant();
    let desk_b = b.participant();
    let _touch = desk_a.seat.get_touch();
    a.roundtrip();
    let (client_b, window_b) = (b.client, desk_b.window.id);

    let _source = start_touch_drag(&server, &mut a, &desk_a);
    let seat_mut = server.call(|state| state.seat_mut);
    let view = server.call(move |state| state.selections.drag(seat_mut));
    let view = view.expect("the drag started");
    assert_eq!(view.device, DragDevice::Touch { slot: SLOT });
    assert_eq!(view.at, Point::new(5.0, 5.0));
    assert!(
        a.received
            .touch
            .ends_with(&["frame".to_owned(), "cancel".to_owned()])
    );
    a.received.take_device();

    touch_move(
        &server,
        SLOT + 1,
        Some((client_b, window_b, (100.0, 0.0))),
        (150.0, 10.0),
    );
    b.roundtrip();
    assert!(b.received.take_device().is_empty());

    touch_move(
        &server,
        SLOT,
        Some((client_b, window_b, (100.0, 0.0))),
        (110.0, 10.0),
    );
    a.roundtrip();
    b.roundtrip();
    assert_eq!(a.received.take_device(), ["leave"]);
    let lines = b.received.take_device();
    let offer_id = offer_in(&lines);
    assert_eq!(
        lines.last().unwrap(),
        &format!("enter {window_b} 10 10 Some({offer_id})")
    );
    let at = server.call(move |state| state.selections.drag(seat_mut).map(|view| view.at));
    assert_eq!(at, Some(Point::new(110.0, 10.0)));

    let offer = b.data_offer(offer_id);
    offer.set_actions(DndAction::COPY, DndAction::COPY);
    offer.accept(0, Some(TEXT));
    b.roundtrip();
    a.roundtrip();

    server.call(|state| {
        state.seat_mut().touch_up(TouchUp {
            slot: SLOT,
            time: Duration::from_millis(3),
        });
    });
    a.roundtrip();
    b.roundtrip();
    assert_eq!(b.received.take_device(), ["drop", "leave"]);
    assert_eq!(
        a.received.data_source.last().map(String::as_str),
        Some("dnd_drop_performed")
    );
    let ended = server.call(move |state| {
        let seat_mut = state.seat_mut;
        let grabbed = state.seats.get(seat_mut).unwrap().touch().grabbed();
        (
            state.has_event(&SelectionEvent::DragEnded {
                seat: seat_mut,
                dropped: true,
            }),
            grabbed,
            state.selections.drag(seat_mut).is_none(),
        )
    });
    assert_eq!(ended, (true, false, true));
}
