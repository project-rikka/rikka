use rikka_compositor::{Extent, Injected};
use rikka_wayland_client::ext_image_copy_capture_manager_v1::{
    ExtImageCopyCaptureManagerV1, Options,
};
use rikka_wayland_client::ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1;
use rikka_wayland_client::wl_shm;

use rikka_wayland_client::zwlr_layer_surface_v1::Anchor;

use crate::common::Harness;

const EXTENT: Extent = Extent {
    width: 128,
    height: 96,
};

const RED_ARGB: [u8; 4] = [0, 0, 255, 255];
const GREEN_ARGB: [u8; 4] = [0, 255, 0, 255];

fn close(pixel: [u8; 4], expected: [u8; 4]) -> bool {
    pixel
        .iter()
        .zip(expected)
        .all(|(channel, expected)| channel.abs_diff(expected) <= 1)
}

#[test]
fn output_shm_capture_reports_damage() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let output = client.output();
    let sources: ExtOutputImageCaptureSourceManagerV1 = client.global(1);
    let copies: ExtImageCopyCaptureManagerV1 = client.global(1);

    let window = client.toplevel(1);
    let surface = window.surface.clone();
    client.ack(&window.xdg, window.tag);
    let paint = client.pixels(16, 16, 1);
    paint.fill(0..16, RED_ARGB);
    surface.frame().set_key(1u32);
    client.show(&surface, &paint);
    client.until("window drawn", |recorded| recorded.frames.contains(&1));
    harness.frame_where(move |snapshot| {
        snapshot
            .pixel(52, 52)
            .is_some_and(|pixel| close(pixel, [255, 0, 0, 255]))
    });

    let source = sources.create_source(&output);
    let session = copies.create_session(&source, Options::empty());
    client.until("capture constraints", |recorded| {
        recorded.captured.described
    });
    assert_eq!(client.received.captured.size, Some((128, 96)));
    assert!(
        client
            .received
            .captured
            .shm_formats
            .contains(&wl_shm::Format::Argb8888)
    );

    let copy = client.pixels(128, 96, 100);
    let frame = session.create_frame().with_key(1u32);
    frame.attach_buffer(&copy.buffer);
    frame.damage_buffer(0, 0, 128, 96);
    frame.capture();
    client.until("first copy", |recorded| {
        recorded.captured.ready.contains(&1)
    });
    assert!(client.received.captured.presented);
    assert!(
        close(copy.read(52, 52), RED_ARGB),
        "{:?}",
        copy.read(52, 52)
    );
    assert!(!close(copy.read(2, 2), RED_ARGB), "{:?}", copy.read(2, 2));
    assert!(client.received.captured.failed.is_empty());

    paint.fill(0..16, GREEN_ARGB);
    surface.attach(Some(&paint.buffer), 0, 0);
    surface.damage_buffer(0, 0, 16, 16);
    surface.commit();
    client.received.captured.damage.clear();
    frame.destroy();
    let frame = session.create_frame().with_key(2u32);
    frame.attach_buffer(&copy.buffer);
    frame.capture();
    client.until("second copy", |recorded| {
        recorded.captured.ready.contains(&2)
    });
    assert!(
        close(copy.read(52, 52), GREEN_ARGB),
        "{:?}",
        copy.read(52, 52)
    );
    let damage = &client.received.captured.damage;
    assert!(
        damage
            .iter()
            .any(|(x, y, w, h)| *x <= 52 && *y <= 52 && x + w > 52 && y + h > 52),
        "{damage:?}"
    );
    assert!(
        damage.iter().all(|(_, _, w, h)| w * h < 128 * 96),
        "damage limited to window: {damage:?}"
    );
}

fn copy_of(
    client: &mut crate::common::client::Client,
    copies: &ExtImageCopyCaptureManagerV1,
    source: &rikka_wayland_client::ext_image_capture_source_v1::ExtImageCaptureSourceV1,
    options: Options,
    tag: u32,
) -> rikka_test_display::Canvas {
    client.received.captured.described = false;
    let session = copies.create_session(source, options);
    client.until("capture constraints", |recorded| {
        recorded.captured.described
    });
    let copy = client.pixels(128, 96, tag);
    let frame = session.create_frame().with_key(tag);
    frame.attach_buffer(&copy.buffer);
    frame.damage_buffer(0, 0, 128, 96);
    frame.capture();
    client.until("the copy", |recorded| {
        recorded.captured.ready.contains(&tag)
    });
    copy
}

fn marked(copy: &rikka_test_display::Canvas, at: (i32, i32)) -> bool {
    let background = copy.read(120, 90);
    (at.1..at.1 + 8).any(|y| (at.0..at.0 + 8).any(|x| copy.read(x, y) != background))
}

#[test]
fn cursor_capture_follows_pointer() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let devices = client.devices();
    let output = client.output();
    let sources: ExtOutputImageCaptureSourceManagerV1 = client.global(1);
    let copies: ExtImageCopyCaptureManagerV1 = client.global(1);
    let source = sources.create_source(&output);

    harness.inject(&[Injected::PointerTo { x: 40.0, y: 30.0 }]);
    let painted = copy_of(&mut client, &copies, &source, Options::PAINT_CURSORS, 1);
    assert!(marked(&painted, (40, 30)), "cursor painted at pointer");

    let _cursor = copies.create_pointer_cursor_session(&source, &devices.pointer);
    client.until("new cursor session reports resting position", |recorded| {
        recorded.captured.cursor_entered && recorded.captured.cursor_at == Some((40, 30))
    });
    assert!(client.received.captured.hotspot.is_some());

    harness.inject(&[Injected::PointerTo { x: 60.0, y: 40.0 }]);
    client.until("cursor session follows pointer", |recorded| {
        recorded.captured.cursor_at == Some((60, 40))
    });

    let plain = copy_of(&mut client, &copies, &source, Options::empty(), 2);
    assert!(!marked(&plain, (60, 40)), "cursor omitted");
}

#[test]
fn output_capture_leaves_out_display_corners() {
    let harness = Harness::full(EXTENT, &[]);
    let mut client = harness.client();
    let output = client.output();
    let sources: ExtOutputImageCaptureSourceManagerV1 = client.global(1);
    let copies: ExtImageCopyCaptureManagerV1 = client.global(1);

    let (backdrop, _layer) = client.layer(1, Anchor::all(), (0, 0), -1);
    let paint = client.pixels(128, 96, 1);
    paint.fill(0..96, RED_ARGB);
    client.show(&backdrop, &paint);
    harness.frame_where(|snapshot| {
        snapshot
            .pixel(64, 48)
            .is_some_and(|pixel| close(pixel, [255, 0, 0, 255]))
            && snapshot
                .pixel(127, 95)
                .is_some_and(|pixel| close(pixel, [0, 0, 0, 255]))
    });

    let source = sources.create_source(&output);
    let copy = copy_of(&mut client, &copies, &source, Options::empty(), 2);
    for at in [(127, 0), (0, 95), (127, 95), (64, 48)] {
        assert!(
            close(copy.read(at.0, at.1), RED_ARGB),
            "{at:?} came back {:?}",
            copy.read(at.0, at.1)
        );
    }
}
