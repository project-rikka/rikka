use std::time::Duration;

use rikka_compositor_geometry::Point as ServerPoint;
use rikka_compositor_seat::{
    Bus as ServerBus, Focus, PadGroupInfo, PadInfo, TabletInfo, ToolMotion,
};
use rikka_compositor_types::{
    PadInput, Press, ToolAxes, ToolCapabilities, ToolInfo, ToolKind as ServerToolKind,
};
use rikka_input::DeviceEvent;
use rikka_input::evdev::{BTN_STYLUS, BTN_STYLUS2};
use rikka_seat::{Bus, PadEvent, SeatEvent};
use rikka_types::{Point, PointerButton, Stylus, Tilt, ToolKind};

use crate::support::{Client, Server};

const TOLERANCE: f32 = 1e-4;

struct Pen {
    tablet: rikka_compositor_seat::TabletId,
    tool: rikka_compositor_seat::ToolId,
}

fn pen(server: &Server, kind: ServerToolKind) -> Pen {
    server.call(move |state| {
        let tablet = state
            .seat()
            .add_tablet(TabletInfo {
                name: "Pen Display".into(),
                vendor: 0x56a,
                product: 0x3b2,
                path: Some("/dev/input/event9".into()),
                bus: Some(ServerBus::Usb),
            })
            .expect("the tablet joins");
        let tool = state
            .seat()
            .add_tool(ToolInfo {
                kind,
                serial: 0x1_0000_0007,
                hardware_id: 0x802,
                capabilities: ToolCapabilities::PRESSURE | ToolCapabilities::TILT,
            })
            .expect("the tool joins");
        Pen { tablet, tool }
    })
}

fn moved(server: &Server, client: &Client, pen: &Pen, at: (f64, f64), axes: ToolAxes, millis: u64) {
    let (tablet, tool, id) = (pen.tablet, pen.tool, client.surface_id());
    server.call(move |state| {
        let surface = state.surface(id);
        state.seat().tool_motion(
            tool,
            tablet,
            ToolMotion {
                location: ServerPoint::new(at.0, at.1),
                under: Some(Focus::new(surface, ServerPoint::new(0.0, 0.0))),
                axes,
                time: Duration::from_millis(millis),
            },
        );
    });
}

fn pressing(pressure: f64) -> ToolAxes {
    ToolAxes {
        pressure: Some(pressure),
        tilt: Some((30.0, -15.0)),
        ..ToolAxes::default()
    }
}

fn stylus_events(events: Vec<SeatEvent>) -> Vec<(DeviceEvent, Stylus)> {
    events
        .into_iter()
        .map(|event| match event {
            SeatEvent::Stylus { event, stylus, .. } => (event, stylus),
            other => panic!("expected a stylus event, got {other:?}"),
        })
        .collect()
}

fn pressures(stylus: &Stylus) -> Vec<f32> {
    stylus
        .samples()
        .iter()
        .map(|sample| sample.pressure.expect("the pen reports pressure"))
        .collect()
}

fn assert_close(actual: &[f32], expected: &[f32]) {
    assert_eq!(actual.len(), expected.len(), "{actual:?} != {expected:?}");
    for (actual, expected) in actual.iter().zip(expected) {
        assert!(
            (actual - expected).abs() < TOLERANCE,
            "{actual} != {expected}"
        );
    }
}

#[test]
fn pen_stroke_becomes_pointer_events_with_coalesced_samples() {
    let server = Server::start();
    let pen = pen(&server, ServerToolKind::Pen);
    let mut client = Client::connect(&server);
    client.take();

    moved(&server, &client, &pen, (10.0, 20.0), pressing(0.0), 1);
    let tool = pen.tool;
    server.call(move |state| {
        state
            .seat()
            .tool_tip(tool, Press::Pressed, Duration::from_millis(2))
    });
    moved(&server, &client, &pen, (12.0, 20.0), pressing(0.25), 3);
    moved(&server, &client, &pen, (14.0, 21.0), pressing(0.5), 4);
    moved(&server, &client, &pen, (16.0, 22.0), pressing(0.75), 5);
    server.call(move |state| {
        let seat = &mut state.seat();
        seat.tool_tip(tool, Press::Released, Duration::from_millis(6));
        seat.tool_out(tool, Duration::from_millis(7));
    });
    let events = stylus_events(client.take());

    let kinds: Vec<&DeviceEvent> = events.iter().map(|(event, _)| event).collect();
    assert!(
        matches!(
            kinds.as_slice(),
            [
                DeviceEvent::PointerMoved(entered),
                DeviceEvent::PointerPressed {
                    button: PointerButton::Primary,
                    ..
                },
                DeviceEvent::PointerMoved(dragged),
                DeviceEvent::PointerReleased {
                    button: PointerButton::Primary,
                    ..
                },
                DeviceEvent::PointerLeft,
            ] if *entered == Point::new(10.0, 20.0) && *dragged == Point::new(16.0, 22.0)
        ),
        "{kinds:?}"
    );
    let (_, hover) = &events[0];
    assert!(!hover.sample().contact);
    assert_eq!(hover.tool.kind, ToolKind::Pen);
    assert_eq!(hover.tool.serial, 0x1_0000_0007);
    assert!(hover.tool.capabilities.pressure && hover.tool.capabilities.tilt);
    let (_, pressed) = &events[1];
    assert!(pressed.sample().contact);
    let (_, drag) = &events[2];
    assert_close(&pressures(drag), &[0.25, 0.5, 0.75]);
    let positions: Vec<Point> = drag
        .samples()
        .iter()
        .map(|sample| sample.position)
        .collect();
    assert_eq!(
        positions,
        [
            Point::new(12.0, 20.0),
            Point::new(14.0, 21.0),
            Point::new(16.0, 22.0)
        ]
    );
    let times: Vec<Duration> = drag.samples().iter().map(|sample| sample.time).collect();
    assert_eq!(times, [3, 4, 5].map(Duration::from_millis));
    assert!(
        drag.samples()
            .iter()
            .all(|sample| sample.contact && sample.tilt == Some(Tilt::new(30.0, -15.0)))
    );
    let (_, released) = &events[3];
    assert!(!released.sample().contact);
}

#[test]
fn flipped_pen_reports_eraser_and_barrel_buttons_map_to_secondary_and_middle() {
    let server = Server::start();
    let pen = pen(&server, ServerToolKind::Eraser);
    let mut client = Client::connect(&server);
    client.take();

    moved(&server, &client, &pen, (5.0, 5.0), pressing(0.0), 1);
    let tool = pen.tool;
    server.call(move |state| {
        let seat = &mut state.seat();
        let at = Duration::from_millis(2);
        seat.tool_button(tool, BTN_STYLUS, Press::Pressed, at);
        seat.tool_button(tool, BTN_STYLUS, Press::Released, at);
        seat.tool_button(tool, BTN_STYLUS2, Press::Pressed, at);
    });
    server.call(move |state| state.seat().tool_out(tool, Duration::from_millis(3)));
    let events = stylus_events(client.take());

    assert!(
        events
            .iter()
            .all(|(_, stylus)| stylus.tool.kind == ToolKind::Eraser && stylus.is_eraser())
    );
    let buttons: Vec<(PointerButton, bool)> = events
        .iter()
        .filter_map(|(event, _)| match event {
            DeviceEvent::PointerPressed { button, .. } => Some((*button, true)),
            DeviceEvent::PointerReleased { button, .. } => Some((*button, false)),
            _ => None,
        })
        .collect();
    assert_eq!(
        buttons,
        [
            (PointerButton::Secondary, true),
            (PointerButton::Secondary, false),
            (PointerButton::Middle, true),
            (PointerButton::Middle, false),
        ],
        "a button held at proximity out is released"
    );
    assert!(matches!(events.last(), Some((DeviceEvent::PointerLeft, _))));
}

#[test]
fn tablets_and_pads_are_described_and_pad_input_reaches_the_focused_surface() {
    let server = Server::start();
    let (pad, _) = server.call(|state| {
        let tablet = state
            .seat()
            .add_tablet(TabletInfo {
                name: "Pen Display".into(),
                vendor: 0x56a,
                product: 0x3b2,
                path: None,
                bus: Some(ServerBus::Usb),
            })
            .expect("the tablet joins");
        let pad = state
            .seat()
            .add_pad(
                tablet,
                PadInfo {
                    path: None,
                    buttons: 4,
                    groups: vec![PadGroupInfo {
                        buttons: vec![0, 1, 2, 3],
                        rings: vec![0],
                        strips: Vec::new(),
                        dials: Vec::new(),
                        modes: 2,
                    }],
                },
            )
            .expect("the pad joins");
        (pad, tablet)
    });
    let mut client = Client::connect(&server);
    client.take();

    let tablet = client
        .seats()
        .tablets()
        .next()
        .expect("the tablet is listed");
    assert_eq!(tablet.name, "Pen Display");
    assert_eq!((tablet.vendor, tablet.product), (0x56a, 0x3b2));
    assert_eq!(tablet.bus, Some(Bus::Usb));
    let described = client.seats().pads().next().expect("the pad is listed");
    assert_eq!(described.buttons, 4);
    assert_eq!(described.groups.len(), 1);
    assert_eq!(described.groups[0].buttons, [0, 1, 2, 3]);
    assert_eq!(described.groups[0].rings, 1);
    assert_eq!(described.groups[0].modes, 2);

    let id = client.surface_id();
    server.call(move |state| {
        let surface = state.surface(id);
        let seat = &mut state.seat();
        seat.pad_focus(pad, Some(surface));
        seat.pad_input(
            pad,
            PadInput::Button {
                button: 2,
                press: Press::Pressed,
            },
            Duration::from_millis(4),
        );
    });
    let events: Vec<PadEvent> = client
        .take()
        .into_iter()
        .filter_map(|event| match event {
            SeatEvent::Pad { event, .. } => Some(event),
            _ => None,
        })
        .collect();
    assert!(events.contains(&PadEvent::Entered), "{events:?}");
    assert!(
        events.contains(&PadEvent::Button {
            button: 2,
            pressed: true,
            time: Duration::from_millis(4),
        }),
        "{events:?}"
    );
}
