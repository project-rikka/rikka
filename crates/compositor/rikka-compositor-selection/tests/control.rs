use std::os::fd::AsFd;

use crate::common::{Server, drain, mimes, pipe};
use rikka_compositor_selection::SelectionKind;
use rikka_wayland_client::ext_data_control_manager_v1;

const TEXT: &str = "text/plain";

#[test]
fn data_control_observes_and_sets_selection() {
    let server = Server::start();
    let mut a = server.connect();
    let mut watcher = server.connect();
    let desk = a.participant();
    let seat = watcher.seat();
    let manager: ext_data_control_manager_v1::ExtDataControlManagerV1 = watcher.global(1);
    let device = manager.get_data_device(&seat);
    watcher.roundtrip();
    assert_eq!(
        watcher.received.control,
        ["selection None", "primary_selection None"]
    );
    watcher.received.control.clear();

    let (client, window) = (a.client, desk.window.id);
    server.call(move |state| state.focus_keyboard(client, window));
    a.roundtrip();
    let copied = a.data_source(&desk.manager, "from a", &[TEXT]);
    desk.device
        .set_selection(Some(&copied), a.received.last_enter_serial());
    a.roundtrip();
    watcher.roundtrip();
    assert_eq!(
        watcher.received.control.len(),
        2,
        "{:?}",
        watcher.received.control
    );
    let offer_id: u32 = watcher.received.control[0]
        .trim_start_matches("data_offer ")
        .parse()
        .unwrap();
    assert_eq!(
        watcher.received.control[1],
        format!("selection Some({offer_id})")
    );
    assert_eq!(watcher.received.offered(offer_id), [TEXT]);
    let (read, write) = pipe();
    watcher.received.control_offers[&offer_id].receive(TEXT, write.as_fd());
    drop(write);
    watcher.roundtrip();
    a.roundtrip();
    assert_eq!(drain(read), "from a");
    watcher.received.control.clear();

    let seat_mut = server.call(|state| state.seat_mut);
    server.call(move |state| {
        rikka_compositor_selection::set_compositor_selection(
            state,
            seat_mut,
            SelectionKind::Primary,
            mimes(&["text/uri-list"]),
        )
        .unwrap();
    });
    watcher.roundtrip();
    assert_eq!(watcher.received.control.len(), 2);
    assert!(watcher.received.control[1].starts_with("primary_selection Some"));
    watcher.received.control.clear();

    let control_source = manager
        .create_data_source()
        .with_key(crate::common::client::Payload::new("from the watcher"));
    control_source.offer(TEXT);
    device.set_selection(Some(&control_source));
    watcher.roundtrip();
    a.roundtrip();
    assert_eq!(
        a.received.data_source.last().map(String::as_str),
        Some("cancelled")
    );
    let line = a.received.device.last().unwrap().clone();
    let offer_id: u32 = line
        .trim_start_matches("selection Some(")
        .trim_end_matches(')')
        .parse()
        .unwrap();
    let (read, write) = pipe();
    a.data_offer(offer_id).receive(TEXT, write.as_fd());
    drop(write);
    a.roundtrip();
    watcher.roundtrip();
    assert_eq!(drain(read), "from the watcher");

    server.call(move |state| {
        state.seat_mut().set_keyboard_focus(None);
        state.focus_keyboard(client, window);
    });
    a.roundtrip();
    let again = a.data_source(&desk.manager, "from a again", &[TEXT]);
    desk.device
        .set_selection(Some(&again), a.received.last_enter_serial());
    a.roundtrip();
    watcher.roundtrip();
    assert_eq!(
        watcher.received.control_source.last().map(String::as_str),
        Some("cancelled")
    );
}

#[test]
fn data_control_is_privileged() {
    let server = Server::start();
    let mut outsider = server.connect_unprivileged();
    outsider.roundtrip();
    assert!(!outsider.advertises("ext_data_control_manager_v1"));
    assert!(outsider.advertises("wl_data_device_manager"));
    assert!(outsider.advertises("zwp_primary_selection_device_manager_v1"));
    let insider = server.connect();
    assert!(insider.advertises("ext_data_control_manager_v1"));
}
