use rikka_wayland_client::Route;
use std::time::Duration;

use rikka_compositor::Extent;
use rikka_seam::Request;
use rikka_session::toplevels::{CAPABILITY, STATE, State};
use rikka_wayland_client::ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1;
use rikka_wayland_client::ext_foreign_toplevel_handle_v1::{self, ExtForeignToplevelHandleV1};
use rikka_wayland_client::ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1;
use rikka_wayland_client::ext_foreign_toplevel_list_v1::{self, ExtForeignToplevelListV1};
use rikka_wayland_client::ext_image_copy_capture_manager_v1::{
    ExtImageCopyCaptureManagerV1, Options,
};
use rikka_wayland_client::ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1;
use rikka_wayland_client::wl_compositor::WlCompositor;
use rikka_wayland_client::zwlr_layer_shell_v1::Layer as Level;
use rikka_wayland_client::zwlr_layer_surface_v1::Anchor;

use rikka_test_display::{Canvas, dispatch_until};

use crate::common::client::Client;
use crate::common::shell::Toplevel;
use crate::common::{Harness, PATIENCE, shows};

const EXTENT: Extent = Extent {
    width: 320,
    height: 240,
};

const GREEN: [u8; 4] = [0, 255, 0, 255];
const RED: [u8; 4] = [0, 0, 255, 255];
const BLUE: [u8; 4] = [255, 0, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const CLEAR: [u8; 4] = [0, 0, 0, 0];
const SHOWN_RED: [u8; 4] = [255, 0, 0, 255];

const APPEARANCE: [(&str, &str); 1] = [("rikka.appearance", "blur = true\nblur-radius = 24.0\n")];

#[derive(Debug, Default)]
struct Listed {
    handles: Vec<(ExtForeignToplevelHandleV1, Option<String>)>,
}

fn on_ext_foreign_toplevel_list_v1(
    listed: &mut Listed,
    _: ExtForeignToplevelListV1,
    event: ext_foreign_toplevel_list_v1::Event,
) {
    if let ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } = event {
        listed.handles.push((toplevel, None));
    }
}

fn on_ext_foreign_toplevel_handle_v1(
    listed: &mut Listed,
    handle: ExtForeignToplevelHandleV1,
    event: ext_foreign_toplevel_handle_v1::Event,
) {
    if let ext_foreign_toplevel_handle_v1::Event::Title { title } = event
        && let Some((_, named)) = listed
            .handles
            .iter_mut()
            .find(|(listed_handle, _)| *listed_handle == handle)
    {
        *named = Some(title);
    }
}

fn mapped(
    client: &mut Client,
    tag: u32,
    title: &str,
    size: (i32, i32),
    bgra: [u8; 4],
) -> (Toplevel, Canvas) {
    let window = client.toplevel(tag);
    window.toplevel.set_title(title);
    client.ack(&window.xdg, tag);
    let pixels = client.pixels(size.0, size.1, tag);
    pixels.fill(0..size.1, bgra);
    client.show(&window.surface, &pixels);
    (window, pixels)
}

fn handle_named(client: &Client, title: &str) -> ExtForeignToplevelHandleV1 {
    let mut queue = client.connection.queue::<Listed>();
    queue.on::<ExtForeignToplevelListV1>(on_ext_foreign_toplevel_list_v1);
    queue.on::<ExtForeignToplevelHandleV1>(on_ext_foreign_toplevel_handle_v1);
    let mut listed = Listed::default();
    let _list = bind_on(client, &queue.route());
    let named = |listed: &Listed| {
        listed
            .handles
            .iter()
            .find(|(_, name)| name.as_deref() == Some(title))
            .map(|(handle, _)| handle.clone())
    };
    dispatch_until(&mut queue, &mut listed, title, PATIENCE, |listed| {
        named(listed).is_some()
    });
    named(&listed).expect("listed toplevel")
}

fn bind_on(client: &Client, route: &Route) -> ExtForeignToplevelListV1 {
    let (name, version) = client
        .globals()
        .iter()
        .find(|global| &*global.interface == "ext_foreign_toplevel_list_v1")
        .map(|global| (global.name, global.version.min(1)))
        .expect("foreign toplevel list global");
    client.registry().via(route).bind(name, version)
}

#[test]
fn window_shm_capture_excludes_overlapping_windows() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let _devices = client.devices();
    let (captured, paint) = mapped(&mut client, 1, "captured", (48, 32), GREEN);
    let (_over, _) = mapped(&mut client, 2, "over", (320, 240), RED);
    harness.frame_where(move |snapshot| {
        snapshot
            .pixel(160, 120)
            .is_some_and(|pixel| pixel == [255, 0, 0, 255])
    });

    let session = session_on(&mut client, "captured");
    assert_eq!(client.received.captured.size, Some((48, 32)));

    let copy = client.pixels(48, 32, 100);
    let frame = session.create_frame().with_key(1u32);
    frame.attach_buffer(&copy.buffer);
    frame.damage_buffer(0, 0, 48, 32);
    frame.capture();
    client.until("window copied", |recorded| {
        recorded.captured.ready.contains(&1) || recorded.captured.failed.contains(&1)
    });
    assert!(client.received.captured.failed.is_empty());
    for (x, y) in [(0, 0), (24, 16), (47, 31)] {
        assert_eq!(copy.read(x, y), GREEN, "at {x},{y}");
    }

    frame.destroy();
    let frame = session.create_frame().with_key(2u32);
    frame.attach_buffer(&copy.buffer);
    frame.capture();
    std::thread::sleep(Duration::from_millis(200));
    client.roundtrip();
    assert!(
        !client.received.captured.ready.contains(&2),
        "unchanged window is not copied"
    );

    paint.fill(0..16, RED);
    captured.surface.attach(Some(&paint.buffer), 0, 0);
    captured.surface.damage_buffer(0, 0, 48, 16);
    captured.surface.commit();
    client.until("change copied", |recorded| {
        recorded.captured.ready.contains(&2)
    });
    assert_eq!(copy.read(24, 4), RED);
    assert_eq!(copy.read(24, 24), GREEN);
}

#[test]
fn translucent_window_capture_shows_only_wallpaper() {
    let harness = Harness::full(EXTENT, &APPEARANCE);
    let mut client = harness.client();
    let compositor: WlCompositor = client.global(6);
    let effects: ExtBackgroundEffectManagerV1 = client.global(1);
    let (wallpaper, _wallpaper_layer) =
        client.layer_on(Level::Background, 1, Anchor::all(), (0, 0), -1);
    let painted = client.pixels(320, 240, 1);
    painted.paint_each(|x, _| if x / 2 % 2 == 0 { BLUE } else { GREEN });
    client.show(&wallpaper, &painted);
    let (widgets, _widgets_layer) = client.layer_on(Level::Bottom, 2, Anchor::all(), (0, 0), -1);
    let furnished = client.pixels(320, 240, 2);
    furnished.fill(0..240, WHITE);
    client.show(&widgets, &furnished);
    let _behind = mapped(&mut client, 3, "behind", (320, 240), RED);

    let window = client.toplevel(4);
    window.toplevel.set_title("glass");
    client.ack(&window.xdg, 4);
    let effect = effects.get_background_effect(&window.surface);
    let frosted = compositor.create_region();
    frosted.add(0, 0, 40, 60);
    effect.set_blur_region(Some(&frosted));
    let clear = client.pixels(80, 60, 4);
    clear.fill(0..60, CLEAR);
    client.show(&window.surface, &clear);
    harness.frame_where(move |snapshot| shows(snapshot, 160, 120, SHOWN_RED));
    let state: State = harness.ask(Request::new(CAPABILITY, STATE));
    let bounds = state
        .toplevels
        .iter()
        .find(|toplevel| toplevel.title == "glass")
        .map(|toplevel| toplevel.bounds)
        .expect("translucent window listed");
    harness.frame_where(move |snapshot| {
        [(20, 30), (60, 30)].iter().all(|(x, y)| {
            shows(
                snapshot,
                (bounds.x + x) as u32,
                (bounds.y + y) as u32,
                SHOWN_RED,
            )
        })
    });

    let session = session_on(&mut client, "glass");
    assert_eq!(client.received.captured.size, Some((80, 60)));
    let copy = client.pixels(80, 60, 100);
    let frame = session.create_frame().with_key(1u32);
    frame.attach_buffer(&copy.buffer);
    frame.damage_buffer(0, 0, 80, 60);
    frame.capture();
    client.until("window copied", |recorded| {
        recorded.captured.ready.contains(&1) || recorded.captured.failed.contains(&1)
    });
    assert!(client.received.captured.failed.is_empty());
    for (x, y) in [(10, 30), (20, 30), (30, 45)] {
        let [blue, green, red, alpha] = copy.read(x, y);
        assert!(
            (60..=200).contains(&blue) && (60..=200).contains(&green) && red <= 4 && alpha == 255,
            "glass blurs only wallpaper at {x},{y}: {:?}",
            [blue, green, red, alpha]
        );
    }
    for (x, y) in [(40, 0), (60, 30), (79, 59)] {
        let pixel = copy.read(x, y);
        assert!(
            [BLUE, GREEN].iter().any(|stripe| pixel
                .iter()
                .zip(stripe)
                .all(|(channel, expected)| channel.abs_diff(*expected) <= 2)),
            "wallpaper sharp through clear parts at {x},{y}: {pixel:?}"
        );
    }
}

fn session_on(client: &mut Client, title: &str) -> ExtImageCopyCaptureSessionV1 {
    let handle = handle_named(client, title);
    let sources: ExtForeignToplevelImageCaptureSourceManagerV1 = client.global(1);
    let copies: ExtImageCopyCaptureManagerV1 = client.global(1);
    let source = sources.create_source(&handle);
    let session = copies.create_session(&source, Options::empty());
    client.until("capture constraints", |recorded| {
        recorded.captured.described
    });
    session
}
