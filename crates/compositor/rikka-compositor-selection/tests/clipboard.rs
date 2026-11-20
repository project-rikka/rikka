use crate::common::{Server, drain, mimes, pipe};
use rikka_compositor_seat::SeatParts;
use rikka_compositor_selection::{SelectionEvent, SelectionKind, SelectionParts, Source};
use rustix::fd::AsFd;

const TEXT: &str = "text/plain;charset=utf-8";

#[test]
fn selection_follows_keyboard_focus() {
    let server = Server::start();
    let mut a = server.connect();
    let mut b = server.connect();
    let desk_a = a.participant();
    let desk_b = b.participant();
    let (client_a, window_a) = (a.client, desk_a.window.id);
    let (client_b, window_b) = (b.client, desk_b.window.id);

    server.call(move |state| state.focus_keyboard(client_a, window_a));
    a.roundtrip();
    let source = a.data_source(&desk_a.manager, "copied by a", &[TEXT]);
    desk_a
        .device
        .set_selection(Some(&source), a.received.last_enter_serial());
    a.roundtrip();
    assert_eq!(
        a.received
            .device
            .last()
            .map(String::as_str)
            .map(|line| line.starts_with("selection Some")),
        Some(true)
    );
    b.roundtrip();
    assert!(b.received.device.is_empty());

    let seat_mut = server.call(move |state| {
        state.focus_keyboard(client_b, window_b);
        state.seat_mut
    });
    b.roundtrip();
    let lines = b.received.take_device();
    assert_eq!(lines.len(), 2, "{lines:?}");
    let offer_id: u32 = lines[0].trim_start_matches("data_offer ").parse().unwrap();
    assert_eq!(lines[1], format!("selection Some({offer_id})"));
    assert_eq!(b.received.offered(offer_id), [TEXT]);

    let (read, write) = pipe();
    b.data_offer(offer_id).receive(TEXT, write.as_fd());
    drop(write);
    b.roundtrip();
    a.roundtrip();
    assert_eq!(drain(read), "copied by a");
    assert_eq!(a.received.data_source, [format!("send {TEXT}")]);

    let current = server.call(move |state| {
        state
            .selections
            .current(seat_mut, SelectionKind::Clipboard)
            .map(|current| (current.source, current.mime_types.to_vec()))
    });
    assert_eq!(
        current,
        Some((Source::Client(client_a), vec![Box::<str>::from(TEXT)]))
    );

    let replacement = b.data_source(&desk_b.manager, "copied by b", &[TEXT]);
    desk_b
        .device
        .set_selection(Some(&replacement), b.received.last_enter_serial());
    b.roundtrip();
    a.roundtrip();
    assert_eq!(
        a.received.data_source.last().map(String::as_str),
        Some("cancelled")
    );
    let changes = server.call(move |state| {
        state
            .events
            .iter()
            .filter(|event| {
                **event
                    == SelectionEvent::Changed {
                        seat: seat_mut,
                        kind: SelectionKind::Clipboard,
                    }
            })
            .count()
    });
    assert_eq!(changes, 2);

    replacement.destroy();
    b.roundtrip();
    assert_eq!(
        b.received.device.last().map(String::as_str),
        Some("selection None")
    );
    assert!(server.call(move |state| {
        state
            .selections
            .current(seat_mut, SelectionKind::Clipboard)
            .is_none()
    }));
}

#[test]
fn set_selection_requires_focus_and_serial() {
    let server = Server::start();
    let mut a = server.connect();
    let desk = a.participant();
    let seat_mut = server.call(|state| state.seat_mut);

    let unfocused = a.data_source(&desk.manager, "nobody", &[TEXT]);
    desk.device.set_selection(Some(&unfocused), 0);
    a.roundtrip();
    assert_eq!(a.received.data_source, ["cancelled"]);
    assert!(server.call(move |state| {
        state
            .selections
            .current(seat_mut, SelectionKind::Clipboard)
            .is_none()
    }));

    server.call(|state| state.trusted = true);
    let allowed = a.data_source(&desk.manager, "allowed", &[TEXT]);
    let serial = server.call(|state| state.display.serial());
    desk.device.set_selection(Some(&allowed), serial);
    a.roundtrip();
    assert!(server.call(move |state| {
        state
            .selections
            .current(seat_mut, SelectionKind::Clipboard)
            .is_some()
    }));
    server.call(|state| state.trusted = false);

    let (client, window) = (a.client, desk.window.id);
    server.call(move |state| state.focus_keyboard(client, window));
    a.roundtrip();
    a.received.data_source.clear();
    let entered = a.received.last_enter_serial();

    let stale = a.data_source(&desk.manager, "stale", &[TEXT]);
    desk.device
        .set_selection(Some(&stale), entered.wrapping_sub(1));
    let future = a.data_source(&desk.manager, "future", &[TEXT]);
    desk.device
        .set_selection(Some(&future), entered.wrapping_add(1000));
    a.roundtrip();
    assert_eq!(a.received.data_source, ["cancelled", "cancelled"]);

    let fresh = a.data_source(&desk.manager, "fresh", &[TEXT]);
    desk.device.set_selection(Some(&fresh), entered);
    a.roundtrip();
    assert_eq!(
        a.received.data_source.last().map(String::as_str),
        Some("cancelled")
    );
    assert_eq!(
        a.received.data_source.len(),
        3,
        "the allowed source is replaced"
    );

    desk.device.set_selection(Some(&allowed), entered);
    assert!(a.fails());
    server.expect("protocol error 1 on wl_data_device");
}

#[test]
fn setting_the_current_source_again_announces_its_added_offers() {
    let server = Server::start();
    let mut a = server.connect();
    let desk = a.participant();
    let (client, window) = (a.client, desk.window.id);
    let seat_mut = server.call(move |state| {
        state.focus_keyboard(client, window);
        state.seat_mut
    });
    a.roundtrip();
    let serial = a.received.last_enter_serial();
    let source = a.data_source(&desk.manager, "copied", &[TEXT]);
    desk.device.set_selection(Some(&source), serial);
    source.offer("SAVE_TARGETS");
    desk.device.set_selection(Some(&source), serial);
    a.roundtrip();

    assert!(
        a.received.data_source.is_empty(),
        "{:?}",
        a.received.data_source
    );
    let offered = server.call(move |state| {
        state
            .selections
            .current(seat_mut, SelectionKind::Clipboard)
            .map(|current| current.mime_types.to_vec())
    });
    assert_eq!(
        offered,
        Some(vec![Box::<str>::from(TEXT), Box::from("SAVE_TARGETS")])
    );
}

const PLAIN: &str = "text/plain";

#[test]
fn host_serves_compositor_source() {
    let server = Server::start();
    let mut a = server.connect();
    let desk = a.participant();
    let (client, window) = (a.client, desk.window.id);
    let seat_mut = server.call(move |state| {
        state.focus_keyboard(client, window);
        state.seat_mut
    });
    a.roundtrip();
    a.received.device.clear();

    let source = server.call(move |state| {
        rikka_compositor_selection::set_compositor_selection(
            state,
            seat_mut,
            SelectionKind::Clipboard,
            mimes(&[PLAIN]),
        )
        .unwrap()
    });
    let current = server.call(move |state| {
        state
            .selections
            .current(seat_mut, SelectionKind::Clipboard)
            .map(|current| current.source)
    });
    assert_eq!(current, Some(Source::Compositor(source)));
    a.roundtrip();
    assert_eq!(a.received.device.len(), 2, "{:?}", a.received.device);
    let offer_id: u32 = a.received.device[0]
        .trim_start_matches("data_offer ")
        .parse()
        .unwrap();
    let (read, write) = pipe();
    a.data_offer(offer_id).receive(PLAIN, write.as_fd());
    drop(write);
    a.roundtrip();
    assert_eq!(drain(read), format!("compositor {PLAIN}"));
    assert_eq!(server.call(|state| state.sent.clone()), [PLAIN]);

    server.call(move |state| {
        rikka_compositor_selection::clear_selection(state, seat_mut, SelectionKind::Clipboard);
    });
    a.roundtrip();
    assert_eq!(
        a.received.device.last().map(String::as_str),
        Some("selection None")
    );
}

#[test]
fn parts_hold_reports_until_flushed() {
    let server = Server::start();
    let mut a = server.connect();
    let desk = a.participant();
    let (client, window) = (a.client, desk.window.id);
    let seat = server.call(move |state| {
        state.focus_keyboard(client, window);
        state.events.clear();
        state.seat_mut
    });
    let changed = SelectionEvent::Changed {
        seat,
        kind: SelectionKind::Clipboard,
    };
    let (read, write) = pipe();
    let (set_reported, flushed_reported, sent_early, sent) = server.call(move |state| {
        let mut parts = SelectionParts::new(
            &mut state.selections,
            &mut state.shell,
            SeatParts::new(&mut state.seats, &mut state.surfaces, &state.display),
            &|_| false,
        );
        rikka_compositor_selection::set_compositor_selection(
            &mut parts,
            seat,
            SelectionKind::Clipboard,
            mimes(&[PLAIN]),
        )
        .unwrap();
        rikka_compositor_selection::request_data(
            &mut parts,
            seat,
            SelectionKind::Clipboard,
            PLAIN.to_owned(),
            write,
        )
        .unwrap();
        let set_reported = state.events.contains(&changed);
        let sent_early = state.sent.clone();
        rikka_compositor_selection::flush(state);
        (
            set_reported,
            state.events.contains(&changed),
            sent_early,
            state.sent.clone(),
        )
    });
    assert!(!set_reported);
    assert!(flushed_reported);
    assert!(sent_early.is_empty());
    assert_eq!(sent, [PLAIN]);
    assert_eq!(drain(read), format!("compositor {PLAIN}"));
}
