use std::time::Duration;

use crate::common::Server;
use crate::common::client::ToplevelConfigured;
use rikka_compositor_geometry::Size;
use rikka_compositor_shell::{PingError, ToplevelConfigure, ToplevelStates, WmCapabilities};
use rikka_wayland_client::wl_shm::Format;

#[test]
fn toplevel_configures_acks_and_maps() {
    let server = Server::start();
    let mut client = server.connect();
    let wm_base = client.wm_base();
    let window = client.toplevel(&wm_base, 1);
    window.toplevel.set_title("Notes");
    window.toplevel.set_app_id("rikka.notes");
    window.surface.commit();
    client.roundtrip();
    server.expect("event: Created");
    server.expect("event: Title");
    server.expect("event: AppId");
    server.expect("event: InitialCommit");

    let id = server.call(|state| state.last_toplevel());
    let serial = server.call(move |state| {
        state
            .shell
            .configure_toplevel(
                id,
                ToplevelConfigure {
                    size: Some(Size::new(640, 480)),
                    states: ToplevelStates::ACTIVATED | ToplevelStates::TILED_LEFT,
                    bounds: Some(Size::new(1920, 1080)),
                    capabilities: Some(WmCapabilities::MAXIMIZE | WmCapabilities::FULLSCREEN),
                    decoration: None,
                },
            )
            .unwrap()
    });
    client.roundtrip();
    assert_eq!(client.received.bounds, vec![(1920, 1080)]);
    assert_eq!(client.received.capabilities, vec![vec![2, 3]]);
    assert_eq!(
        client.received.toplevel_configures,
        vec![ToplevelConfigured {
            tag: 1,
            width: 640,
            height: 480,
            states: vec![4, 5],
        }]
    );
    assert_eq!(client.received.last_configure(1), serial);
    let awaiting = server.call(move |state| state.shell.toplevel(id).unwrap().awaiting_ack());
    assert!(awaiting);

    window.xdg_surface.ack_configure(serial);
    window.xdg_surface.set_window_geometry(10, 10, 620, 460);
    let buffer = client.canvas(640, 480, Format::Argb8888);
    window.surface.attach(Some(&buffer.buffer), 0, 0);
    window.surface.commit();
    client.roundtrip();
    server.expect(&format!("event: Configured {{ serial: {serial} }}"));
    server.expect("event: Mapped");
    let (mapped, size, geometry, title, awaiting) = server.call(move |state| {
        let toplevel = state.shell.toplevel(id).unwrap();
        (
            toplevel.mapped(),
            toplevel.current().and_then(|configure| configure.size),
            toplevel
                .window_geometry()
                .map(|rect| (rect.loc.x, rect.size.w)),
            toplevel.title().map(str::to_owned),
            toplevel.awaiting_ack(),
        )
    });
    assert!(mapped);
    assert_eq!(size, Some(Size::new(640, 480)));
    assert_eq!(geometry, Some((10, 620)));
    assert_eq!(title.as_deref(), Some("Notes"));
    assert!(!awaiting);

    window.surface.attach(None, 0, 0);
    window.surface.commit();
    client.roundtrip();
    server.expect("event: Unmapped");
    let (mapped, title, sent) = server.call(move |state| {
        let toplevel = state.shell.toplevel(id).unwrap();
        (
            toplevel.mapped(),
            toplevel.title().map(str::to_owned),
            toplevel.initial_configure_sent(),
        )
    });
    assert!(!mapped);
    assert_eq!(title, None);
    assert!(!sent);

    window.surface.commit();
    client.roundtrip();
    server.expect("event: InitialCommit");
}

#[test]
fn unconfigured_toplevel_buffer_is_rejected() {
    let server = Server::start();
    let mut client = server.connect();
    let wm_base = client.wm_base();
    let window = client.toplevel(&wm_base, 1);
    let buffer = client.canvas(10, 10, Format::Argb8888);
    window.surface.attach(Some(&buffer.buffer), 0, 0);
    window.surface.commit();
    assert!(client.fails());
    server.expect("protocol error 3 on xdg_surface");
}

#[test]
fn parent_loops_are_rejected() {
    let server = Server::start();
    let mut client = server.connect();
    let wm_base = client.wm_base();
    let first = client.toplevel(&wm_base, 1);
    let second = client.toplevel(&wm_base, 2);
    second.toplevel.set_parent(Some(&first.toplevel));
    client.roundtrip();
    server.expect("event: Parent");
    let (first_id, parent) = server.call(|state| {
        let first = state.toplevels[0];
        let second = state.toplevels[1];
        (first, state.shell.toplevel(second).unwrap().parent())
    });
    assert_eq!(parent, Some(first_id));

    first.toplevel.set_parent(Some(&second.toplevel));
    assert!(client.fails());
    server.expect("protocol error 1 on xdg_toplevel");
}

#[test]
fn unanswered_ping_marks_unresponsive() {
    let server = Server::start();
    let mut client = server.connect();
    let wm_base = client.wm_base();
    client.roundtrip();
    let who = server.call(|state| state.display.clients()[0].id());
    let serial = server
        .call(move |state| state.shell.ping(who, Duration::from_millis(20)))
        .unwrap();
    let again = server.call(move |state| state.shell.ping(who, Duration::from_millis(20)));
    assert_eq!(again, Err(PingError::Unanswered(serial)));
    client.roundtrip();
    assert_eq!(client.received.pings, vec![serial]);
    server.expect("Unresponsive");
    assert!(server.call(move |state| state.shell.unresponsive(who)));

    wm_base.pong(serial);
    client.roundtrip();
    server.expect("Pong");
    assert!(!server.call(move |state| state.shell.unresponsive(who)));
}

#[test]
fn acks_dropped_configures() {
    let server = Server::start();
    let mut client = server.connect();
    let wm_base = client.wm_base();
    let window = client.toplevel(&wm_base, 1);
    window.surface.commit();
    client.roundtrip();
    server.expect("event: InitialCommit");

    let id = server.call(|state| state.last_toplevel());
    let serials: Vec<u32> = server.call(move |state| {
        (1..=100)
            .map(|width| {
                state
                    .shell
                    .configure_toplevel(
                        id,
                        ToplevelConfigure {
                            size: Some(Size::new(width, 50)),
                            ..ToplevelConfigure::default()
                        },
                    )
                    .unwrap()
            })
            .collect()
    });
    client.roundtrip();

    window.xdg_surface.ack_configure(serials[0]);
    let buffer = client.canvas(1, 50, Format::Argb8888);
    window.surface.attach(Some(&buffer.buffer), 0, 0);
    window.surface.commit();
    client.roundtrip();
    server.expect("event: Mapped");

    let last = serials[99];
    window.xdg_surface.ack_configure(last);
    window.surface.commit();
    client.roundtrip();
    server.expect(&format!("event: Configured {{ serial: {last} }}"));
    let size = server.call(move |state| {
        state
            .shell
            .toplevel(id)
            .unwrap()
            .current()
            .and_then(|configure| configure.size)
    });
    assert_eq!(size, Some(Size::new(100, 50)));

    window.xdg_surface.ack_configure(serials[1]);
    assert!(client.fails());
    server.expect("protocol error 4 on xdg_surface");
}
