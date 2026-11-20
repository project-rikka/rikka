use std::time::Duration;

use crate::common::Server;
use rikka_compositor_geometry::Point;
use rikka_compositor_seat::{Bus, Focus, PadGroupInfo, PadInfo, TabletInfo, ToolMotion};
use rikka_compositor_types::{
    PadInput, PadSource, Press, ToolAxes, ToolCapabilities, ToolInfo, ToolKind,
};
use rikka_wayland_client::{
    zwp_tablet_manager_v2, zwp_tablet_pad_group_v2, zwp_tablet_pad_v2, zwp_tablet_seat_v2,
    zwp_tablet_tool_v2, zwp_tablet_v2,
};

#[test]
fn tablet_seat_tracks_devices_and_tool() {
    let server = Server::start();
    let mut client = server.connect();
    let seat = client.seat(0);
    let target = client.mapped(40, 40);
    let (tablet, tool, pad) = server.call(|state| {
        let tablet = state
            .seat_mut()
            .add_tablet(TabletInfo {
                name: "Pen Display".into(),
                vendor: 0x56a,
                product: 0x3b2,
                path: Some("/dev/input/event9".into()),
                bus: Some(Bus::Usb),
            })
            .unwrap();
        let tool = state
            .seat_mut()
            .add_tool(ToolInfo {
                kind: ToolKind::Pen,
                serial: 0x1_0000_0007,
                hardware_id: 0,
                capabilities: ToolCapabilities::PRESSURE | ToolCapabilities::TILT,
            })
            .unwrap();
        let pad = state
            .seat_mut()
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
            .unwrap();
        (tablet, tool, pad)
    });
    let manager: zwp_tablet_manager_v2::ZwpTabletManagerV2 = client.global(2);
    let _tablet_seat = manager.get_tablet_seat(&seat);
    client.roundtrip();

    assert!(matches!(
        client.received.tablet_seat.as_slice(),
        [
            zwp_tablet_seat_v2::Event::TabletAdded { .. },
            zwp_tablet_seat_v2::Event::ToolAdded { .. },
            zwp_tablet_seat_v2::Event::PadAdded { .. },
        ]
    ));
    assert!(client.received.tablet.iter().any(|event| matches!(
        event,
        zwp_tablet_v2::Event::Name { name } if name == "Pen Display"
    )));
    assert!(client.received.tablet.iter().any(|event| matches!(
        event,
        zwp_tablet_v2::Event::Id {
            vid: 0x56a,
            pid: 0x3b2
        }
    )));
    assert!(matches!(
        client.received.tool.as_slice(),
        [
            zwp_tablet_tool_v2::Event::Type {
                tool_type: Ok(zwp_tablet_tool_v2::Type::Pen)
            },
            zwp_tablet_tool_v2::Event::HardwareSerial {
                hardware_serial_hi: 1,
                hardware_serial_lo: 7
            },
            zwp_tablet_tool_v2::Event::Capability { .. },
            zwp_tablet_tool_v2::Event::Capability { .. },
            zwp_tablet_tool_v2::Event::Done,
        ]
    ));
    assert!(
        client
            .received
            .pad
            .iter()
            .any(|event| matches!(event, zwp_tablet_pad_v2::Event::Buttons { buttons: 4 }))
    );
    assert!(
        client
            .received
            .pad_group
            .iter()
            .any(|event| matches!(event, zwp_tablet_pad_group_v2::Event::Modes { modes: 2 }))
    );
    client.received.tool.clear();

    let id = target.id;
    server.call(move |state| {
        let surface = state.surface(id);
        let under = Some(Focus::new(surface, Point::new(100.0, 100.0)));
        state.seat_mut().tool_motion(
            tool,
            tablet,
            ToolMotion {
                location: Point::new(110.0, 120.0),
                under,
                axes: ToolAxes {
                    pressure: Some(0.5),
                    ..ToolAxes::default()
                },
                time: Duration::from_millis(5),
            },
        );
        state
            .seat_mut()
            .tool_tip(tool, Press::Pressed, Duration::from_millis(6));
        state.seat_mut().tool_out(tool, Duration::from_millis(7));
        state.seat_mut().pad_focus(pad, Some(surface));
        state.seat_mut().pad_input(
            pad,
            PadInput::Ring {
                ring: 0,
                degrees: Some(45.0),
                source: PadSource::Finger,
            },
            Duration::from_millis(8),
        );
    });
    client.roundtrip();

    let tool_events = &client.received.tool;
    assert!(matches!(
        &tool_events[0],
        zwp_tablet_tool_v2::Event::ProximityIn { surface, .. } if surface.id() == id
    ));
    assert!(matches!(
        tool_events[1],
        zwp_tablet_tool_v2::Event::Motion { x, y } if x == 10.0 && y == 20.0
    ));
    assert!(matches!(
        tool_events[2],
        zwp_tablet_tool_v2::Event::Pressure { pressure: 32767 }
    ));
    assert!(matches!(
        tool_events[3],
        zwp_tablet_tool_v2::Event::Frame { time: 5 }
    ));
    assert!(matches!(
        tool_events[4],
        zwp_tablet_tool_v2::Event::Down { .. }
    ));
    assert!(matches!(
        tool_events[5],
        zwp_tablet_tool_v2::Event::Frame { .. }
    ));
    assert!(matches!(tool_events[6], zwp_tablet_tool_v2::Event::Up));
    assert!(matches!(
        tool_events[7],
        zwp_tablet_tool_v2::Event::ProximityOut
    ));
    assert!(
        client
            .received
            .pad
            .iter()
            .any(|event| matches!(event, zwp_tablet_pad_v2::Event::Enter { .. }))
    );
    assert!(client.received.pad_group.iter().any(|event| matches!(
        event,
        zwp_tablet_pad_group_v2::Event::ModeSwitch { mode: 0, .. }
    )));
}
