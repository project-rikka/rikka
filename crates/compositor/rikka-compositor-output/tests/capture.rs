use rikka_compositor_types::OutputId;

use crate::common::{PRESENTED, Server, State, Wayland};
use rikka_compositor_geometry::Size;
use rikka_compositor_output::{CaptureFailure, CaptureSource, OutputInfo};
use rikka_wayland_client::{
    ext_image_capture_source_v1, ext_image_copy_capture_frame_v1,
    ext_image_copy_capture_manager_v1, ext_image_copy_capture_session_v1,
    ext_output_image_capture_source_manager_v1, wl_output, wl_shm,
};

type Session = ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1;
type Frame = ext_image_copy_capture_frame_v1::ExtImageCopyCaptureFrameV1;
type Manager = ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1;

const CAPTURE_GLOBALS: [&str; 3] = [
    "ext_image_copy_capture_manager_v1",
    "ext_output_image_capture_source_manager_v1",
    "ext_foreign_toplevel_image_capture_source_manager_v1",
];

fn add_output(server: &Server) -> OutputId {
    server.call(|state: &mut State| {
        let display = state.display.clone();
        let output = state.outputs.connect("DP-1");
        state
            .outputs
            .publish(&display, output, OutputInfo::default());
        output
    })
}

fn output_source(client: &mut Wayland) -> ext_image_capture_source_v1::ExtImageCaptureSourceV1 {
    let output: wl_output::WlOutput = client.global(4);
    let sources: ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1 =
        client.global(1);
    sources.create_source(&output)
}

fn output_session(client: &mut Wayland) -> (Manager, Session) {
    let source = output_source(client);
    let manager: Manager = client.global(1);
    let session =
        manager.create_session(&source, ext_image_copy_capture_manager_v1::Options::empty());
    client.roundtrip();
    (manager, session)
}

#[test]
fn capture_globals_are_privileged() {
    let server = Server::start();
    let privileged = server.connect();
    let restricted = server.connect_unprivileged();
    for global in CAPTURE_GLOBALS {
        assert!(privileged.advertises(global), "{global}");
        assert!(!restricted.advertises(global), "{global}");
    }
}

#[test]
fn captured_frame_reports_ready() {
    let server = Server::start();
    add_output(&server);
    server.call(|state| state.copy_with = Some(0xab));
    let mut client = server.connect();
    let (_, session) = output_session(&mut client);
    let target = client.canvas(4, 2, wl_shm::Format::Argb8888);

    let frame: Frame = session.create_frame();
    frame.attach_buffer(&target.buffer);
    frame.damage_buffer(0, 0, 4, 2);
    frame.capture();
    client.roundtrip();

    assert_eq!(
        client.events(&frame),
        [
            "Transform { transform: Ok(_90) }",
            "Damage { x: 0, y: 0, width: 4, height: 2 }",
            format!(
                "PresentationTime {{ tv_sec_hi: 1, tv_sec_lo: 2, tv_nsec: {} }}",
                PRESENTED.subsec_nanos()
            )
            .as_str(),
            "Ready",
        ]
    );
    assert!(
        (0..2)
            .flat_map(|y| (0..4).map(move |x| (x, y)))
            .all(|(x, y)| target.read(x, y) == [0xab; 4])
    );
    let requested = server.call(|state| {
        state
            .requests
            .iter()
            .map(|request| (request.size, request.damage.iter().count()))
            .collect::<Vec<_>>()
    });
    assert_eq!(requested, [(Size::new(4, 2), 1)]);

    frame.destroy();
    let again: Frame = session.create_frame();
    again.attach_buffer(&target.buffer);
    again.capture();
    client.roundtrip();
    assert!(
        client
            .events(&again)
            .contains(&"Damage { x: 0, y: 0, width: 2, height: 2 }".to_owned())
    );
}

#[test]
fn unconstrained_buffer_fails_frame() {
    let server = Server::start();
    add_output(&server);
    let mut client = server.connect();
    let (_, session) = output_session(&mut client);
    let wrong_size = client.canvas(8, 8, wl_shm::Format::Argb8888);

    let frame: Frame = session.create_frame();
    frame.attach_buffer(&wrong_size.buffer);
    frame.capture();
    client.roundtrip();

    assert_eq!(
        client.events(&frame),
        ["Failed { reason: Ok(BufferConstraints) }"]
    );
    assert!(server.call(|state| state.requests.is_empty()));
}

#[test]
fn second_pending_frame_is_protocol_error() {
    let server = Server::start();
    add_output(&server);
    let mut client = server.connect();
    let (_, session) = output_session(&mut client);
    let _first: Frame = session.create_frame();
    let _second: Frame = session.create_frame();
    assert!(client.fails());
}

#[test]
fn stopped_source_fails_frame_and_session() {
    let server = Server::start();
    let output = add_output(&server);
    let mut client = server.connect();
    let (_, session) = output_session(&mut client);
    let target = client.canvas(4, 2, wl_shm::Format::Argb8888);
    let frame: Frame = session.create_frame();
    frame.attach_buffer(&target.buffer);
    frame.capture();
    client.roundtrip();
    assert!(server.call(|state| state.captures.is_pending(state.requests[0].frame)));
    client.forget_events();

    server.call(move |state| state.captures.stop_source(CaptureSource::Output(output)));
    client.roundtrip();
    assert_eq!(client.events(&frame), ["Failed { reason: Ok(Stopped) }"]);
    assert_eq!(client.events(&session), ["Stopped"]);
    assert!(!server.call(|state| state.captures.is_pending(state.requests[0].frame)));
    let late = server.call(|state| {
        let frame = state.requests[0].frame;
        state.captures.fail(frame, CaptureFailure::Unknown)
    });
    assert!(!late);

    frame.destroy();
    let after: Frame = session.create_frame();
    after.attach_buffer(&target.buffer);
    after.capture();
    client.roundtrip();
    assert_eq!(client.events(&after), ["Failed { reason: Ok(Stopped) }"]);
}
