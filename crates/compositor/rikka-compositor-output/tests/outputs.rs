use rikka_compositor_types::OutputId;

use crate::common::{Server, State};
use rikka_compositor_geometry::{Point, Size, Transform};
use rikka_compositor_output::{Millimetres, Mode, OutputInfo, Publication, Subpixel};
use rikka_wayland_client::{wl_output, zxdg_output_manager_v1};

fn panel() -> OutputInfo {
    OutputInfo {
        description: "Rikka Panel 27\"".to_owned(),
        make: "Rikka".to_owned(),
        model: "Panel".to_owned(),
        serial: Some("0042".to_owned()),
        physical_size: Millimetres {
            width: 600,
            height: 340,
        },
        subpixel: Subpixel::HorizontalRgb,
        transform: Transform::Normal,
        mode: Mode {
            size: Size::new(2560, 1440),
            refresh_millihertz: 60_000,
        },
        scale: 1.5,
        position: Point::new(100, 0),
    }
}

fn add_panel(server: &Server) -> OutputId {
    server.call(|state: &mut State| {
        let display = state.display.clone();
        let output = state.outputs.connect("DP-1");
        state.outputs.publish(&display, output, panel());
        output
    })
}

fn kinds(events: &[String]) -> Vec<String> {
    events
        .iter()
        .map(|event| {
            event
                .split([' ', '{', '('])
                .next()
                .unwrap_or_default()
                .to_owned()
        })
        .collect()
}

#[test]
fn bound_output_sends_state_then_done() {
    let server = Server::start();
    add_panel(&server);
    let mut client = server.connect();
    let output: wl_output::WlOutput = client.global(4);
    client.roundtrip();

    let events = client.events(&output);
    assert_eq!(
        kinds(&events),
        ["Geometry", "Mode", "Name", "Description", "Scale", "Done"]
    );
    assert!(events[0].contains("x: 100") && events[0].contains("physical_width: 600"));
    assert!(events[0].contains("\"Rikka\"") && events[0].contains("\"Panel\""));
    assert!(events[1].contains("width: 2560") && events[1].contains("refresh: 60000"));
    assert_eq!(events[2], "Name { name: \"DP-1\" }");
    assert_eq!(events[4], "Scale { factor: 2 }");
    server.expect("bound output v4");
}

#[test]
fn old_output_versions_get_known_events() {
    let server = Server::start();
    add_panel(&server);
    let mut client = server.connect();
    let first: wl_output::WlOutput = client.global(1);
    let second: wl_output::WlOutput = client.global(2);
    let third: wl_output::WlOutput = client.global(3);
    client.roundtrip();

    assert_eq!(kinds(&client.events(&first)), ["Geometry", "Mode"]);
    assert_eq!(
        kinds(&client.events(&second)),
        ["Geometry", "Mode", "Scale", "Done"]
    );
    assert_eq!(
        kinds(&client.events(&third)),
        ["Geometry", "Mode", "Scale", "Done"]
    );
}

#[test]
fn output_change_sends_delta_then_done() {
    let server = Server::start();
    let id = add_panel(&server);
    let mut client = server.connect();
    let output: wl_output::WlOutput = client.global(4);
    client.roundtrip();
    client.forget_events();

    server.call(move |state| {
        let mut info = panel();
        info.position = Point::new(0, 0);
        let display = state.display.clone();
        state.outputs.publish(&display, id, info);
    });
    client.roundtrip();
    assert_eq!(kinds(&client.events(&output)), ["Geometry", "Done"]);
    client.forget_events();

    server.call(move |state| {
        let mut info = panel();
        info.position = Point::new(0, 0);
        info.mode.refresh_millihertz = 144_000;
        info.scale = 1.0;
        let display = state.display.clone();
        state.outputs.publish(&display, id, info);
    });
    client.roundtrip();
    assert_eq!(kinds(&client.events(&output)), ["Mode", "Scale", "Done"]);
    client.forget_events();

    server.call(move |state| {
        let mut info = panel();
        info.position = Point::new(0, 0);
        info.mode.refresh_millihertz = 144_000;
        info.scale = 1.0;
        let display = state.display.clone();
        state.outputs.publish(&display, id, info);
    });
    client.roundtrip();
    assert!(client.events(&output).is_empty());
}

#[test]
fn xdg_output_reports_logical_geometry() {
    let server = Server::start();
    let id = add_panel(&server);
    let mut client = server.connect();
    let output: wl_output::WlOutput = client.global(4);
    let manager: zxdg_output_manager_v1::ZxdgOutputManagerV1 = client.global(3);
    client.roundtrip();
    client.forget_events();

    let xdg = manager.get_xdg_output(&output);
    client.roundtrip();
    let events = client.events(&xdg);
    assert_eq!(
        kinds(&events),
        ["LogicalPosition", "LogicalSize", "Name", "Description"]
    );
    assert_eq!(events[0], "LogicalPosition { x: 100, y: 0 }");
    assert_eq!(events[1], "LogicalSize { width: 1707, height: 960 }");
    assert_eq!(kinds(&client.events(&output)), ["Done"]);
    client.forget_events();

    server.call(move |state| {
        let mut info = panel();
        info.scale = 1.75;
        let display = state.display.clone();
        state.outputs.publish(&display, id, info);
    });
    client.roundtrip();
    assert_eq!(
        client.events(&xdg),
        ["LogicalSize { width: 1463, height: 823 }"]
    );
    assert_eq!(kinds(&client.events(&output)), ["Done"]);
}

#[test]
fn head_keeps_its_id_while_withdrawn_and_published_again() {
    let server = Server::start();
    let id = server.call(|state: &mut State| state.outputs.connect("DP-1"));
    let mut client = server.connect();
    client.roundtrip();
    assert!(!client.advertises("wl_output"));

    let published = server.call(move |state| {
        let display = state.display.clone();
        [
            state.outputs.publish(&display, id, panel()),
            state.outputs.publish(&display, id, panel()),
        ]
    });
    assert_eq!(published, [Publication::Published, Publication::Unchanged]);
    client.roundtrip();
    let stale: wl_output::WlOutput = client.global(4);
    client.roundtrip();

    server.call(move |state| {
        assert!(state.outputs.withdraw(id));
        let display = state.display.clone();
        state.outputs.publish(&display, id, panel())
    });
    client.roundtrip();
    let fresh: wl_output::WlOutput = client.global(4);
    client.roundtrip();
    let (stale, fresh) = (stale.id(), fresh.id());
    let resolved = server.call(move |state| (state.output_of(stale), state.output_of(fresh)));
    assert_eq!(resolved, (None, Some(id)));
}
