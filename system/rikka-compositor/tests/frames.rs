use std::time::Duration;

use rikka_compositor::{Extent, Snapshot};
use rikka_wayland_client::ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1;
use rikka_wayland_client::wl_compositor::WlCompositor;
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::wp_alpha_modifier_v1::WpAlphaModifierV1;
use rikka_wayland_client::wp_fifo_manager_v1::WpFifoManagerV1;
use rikka_wayland_client::zwlr_layer_surface_v1::{Anchor, ZwlrLayerSurfaceV1};

use crate::common::client::Client;
use crate::common::{Harness, Presented, shows};

const EXTENT: Extent = Extent {
    width: 320,
    height: 240,
};

const SMALL: Extent = Extent {
    width: 96,
    height: 64,
};

const BACKGROUND: [u8; 4] = [15, 15, 20, 255];

const RED: [u8; 4] = [0, 0, 255, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [255, 0, 0, 255];
const CLEAR: [u8; 4] = [0, 0, 0, 0];
const SHOWN_RED: [u8; 4] = [255, 0, 0, 255];
const SHOWN_BLUE: [u8; 4] = [0, 0, 255, 255];
const SHOWN_GREEN: [u8; 4] = [0, 255, 0, 255];

const AT: (i32, i32) = (48, 48);

const KEY_LEFTMETA: u32 = 125;
const KEY_1: u32 = 2;
const KEY_2: u32 = 3;

const UNSEEN_AND_MORE: Duration = Duration::from_millis(1500);

const APPEARANCE: [(&str, &str); 1] = [("rikka.appearance", "blur = true\nblur-radius = 24.0\n")];
const UNBLURRED: [(&str, &str); 1] = [("rikka.appearance", "blur = false\n")];

const EDGE: u32 = 120;
const INSIDE: u32 = 160;
const OUTSIDE: u32 = 20;

fn cursor_drawn(snapshot: &Snapshot) -> bool {
    (0..24).any(|y| (0..24).any(|x| !shows(snapshot, x, y, BACKGROUND)))
}

fn inside(x: i32, y: i32) -> (u32, u32) {
    ((AT.0 + x) as u32, (AT.1 + y) as u32)
}

fn anywhere(snapshot: &Snapshot, expected: [u8; 4]) -> bool {
    (0..EXTENT.height).step_by(4).any(|y| {
        (0..EXTENT.width)
            .step_by(4)
            .any(|x| shows(snapshot, x, y, expected))
    })
}

fn blended(snapshot: &Snapshot, x: u32, y: u32) -> bool {
    snapshot
        .pixel(x, y)
        .is_some_and(|[red, _, blue, _]| red > 40 && blue > 40)
}

fn whole_layer(client: &mut Client, tag: u32) -> (WlSurface, ZwlrLayerSurfaceV1) {
    client.layer(tag, Anchor::all(), (0, 0), -1)
}

#[test]
fn session_shows_background_and_cursor() {
    let harness = Harness::headless(SMALL);

    let globals = harness.globals();
    assert!(
        globals.iter().any(|global| global == "wl_fixes"),
        "registry globals: {globals:?}"
    );

    let frame = harness.frame_where(move |snapshot| shows(snapshot, 80, 50, BACKGROUND));
    assert_eq!((frame.width(), frame.height()), (SMALL.width, SMALL.height));
    assert!(shows(&frame, SMALL.width - 1, SMALL.height - 1, BACKGROUND));
    assert!(cursor_drawn(&frame), "cursor composited at top-left");
    assert_eq!(frame.pixel(SMALL.width, 0), None);
}

#[test]
fn client_damage_repaints_only_damaged_region() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    assert!(client.advertises("wl_subcompositor") && client.advertises("wp_viewporter"));

    let window = client.toplevel(1);
    let surface = window.surface.clone();
    client.ack(&window.xdg, window.tag);
    let pixels = client.pixels(32, 24, 1);
    pixels.fill(0..24, RED);
    surface.frame().set_key(1u32);
    client.presentation.feedback(&surface).set_key(1u32);
    client.show(&surface, &pixels);

    let (x, y) = inside(12, 12);
    let first = harness.frame_where(move |snapshot| shows(snapshot, x, y, SHOWN_RED));
    assert!(!shows(&first, 5, 5, SHOWN_RED), "background around cursor");
    let (right, bottom) = inside(32, 24);
    assert!(
        !shows(&first, right, bottom, SHOWN_RED),
        "cursor ends where expected"
    );

    client.until("first frame callback", |recorded| {
        recorded.frames.contains(&1)
            && recorded
                .presented
                .iter()
                .any(|presented| presented.tag == 1)
    });
    client.until("buffer released", |recorded| recorded.released.contains(&1));

    pixels.fill(0..24, GREEN);
    pixels.fill(8..12, BLUE);
    surface.attach(Some(&pixels.buffer), 0, 0);
    surface.damage_buffer(0, 8, 32, 4);
    surface.frame().set_key(2u32);
    client.presentation.feedback(&surface).set_key(2u32);
    surface.commit();
    client.roundtrip();

    let (band_x, band_y) = inside(12, 9);
    let patched = harness.frame_where(move |snapshot| shows(snapshot, band_x, band_y, SHOWN_BLUE));
    let (above_x, above_y) = inside(12, 4);
    assert!(
        shows(&patched, above_x, above_y, SHOWN_RED),
        "undamaged rows not re-uploaded"
    );
    let (below_x, below_y) = inside(12, 20);
    assert!(shows(&patched, below_x, below_y, SHOWN_RED));

    client.until("second frame callback", |recorded| {
        recorded.frames.contains(&2)
            && recorded
                .presented
                .iter()
                .any(|presented| presented.tag == 2)
    });
    let sequences: Vec<u64> = [1, 2]
        .iter()
        .filter_map(|tag| {
            client
                .received
                .presented
                .iter()
                .find(|presented: &&Presented| presented.tag == *tag)
                .map(|presented| presented.sequence)
        })
        .collect();
    assert!(sequences[0] < sequences[1], "{sequences:?}");
    assert!(client.received.discarded.is_empty());

    pixels.paint_each(|_, _| BLUE);
    surface.attach(Some(&pixels.buffer), 0, 0);
    surface.damage_buffer(4, 4, 4, 4);
    surface.damage_buffer(24, 16, 4, 4);
    surface.commit();
    client.roundtrip();
    let (far_x, far_y) = inside(25, 17);
    let corners = harness.frame_where(move |snapshot| shows(snapshot, far_x, far_y, SHOWN_BLUE));
    let (near_x, near_y) = inside(5, 5);
    assert!(shows(&corners, near_x, near_y, SHOWN_BLUE));
    let (between_x, between_y) = inside(14, 14);
    assert!(
        shows(&corners, between_x, between_y, SHOWN_RED),
        "gap between damaged rects not uploaded"
    );
}

#[test]
fn hidden_workspace_surface_gets_no_frames() {
    let harness = Harness::full(EXTENT, &[]);
    let mut client = harness.client();
    let _devices = client.devices();
    let window = client.toplevel(1);
    client.ack(&window.xdg, window.tag);
    let pixels = client.pixels(32, 24, 1);
    pixels.fill(0..24, RED);
    client.show(&window.surface, &pixels);
    harness.frame_where(move |snapshot| anywhere(snapshot, SHOWN_RED));

    harness.press(&[KEY_LEFTMETA, KEY_2]);
    harness.frame_where(move |snapshot| !anywhere(snapshot, SHOWN_RED));
    window.surface.frame().set_key(7u32);
    window.surface.commit();
    let roleless = client.surface();
    roleless.attach(Some(&pixels.buffer), 0, 0);
    roleless.frame().set_key(8u32);
    roleless.commit();
    client.roundtrip();
    std::thread::sleep(UNSEEN_AND_MORE);
    client.roundtrip();
    assert!(
        !client.received.frames.contains(&7) && !client.received.frames.contains(&8),
        "{:?}",
        client.received.frames
    );

    harness.press(&[KEY_LEFTMETA, KEY_1]);
    client.until("frame callback once shown", |recorded| {
        recorded.frames.contains(&7)
    });
    assert!(!client.received.frames.contains(&8));
}

#[test]
fn late_bound_output_gets_enter_and_feedback() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();

    let window = client.toplevel(1);
    let surface = window.surface.clone();
    client.ack(&window.xdg, window.tag);
    let pixels = client.pixels(16, 16, 1);
    pixels.fill(0..16, [0, 0, 255, 255]);
    surface.frame().set_key(1u32);
    client.show(&surface, &pixels);
    client.until("first frame", |recorded| recorded.frames.contains(&1));

    let output = client.output();
    let bound = output.id();
    client.until("output described", |recorded| {
        recorded.outputs.iter().any(|described| described.done)
    });
    let described = &client.received.outputs[0];
    assert_eq!(
        (described.make.as_str(), described.model.as_str()),
        ("Rikka", "Headless")
    );
    assert_eq!(described.mode, (320, 240));
    client.until("surface enters output", |recorded| {
        recorded.entered.contains(&bound)
    });

    pixels.fill(0..16, [0, 255, 0, 255]);
    surface.attach(Some(&pixels.buffer), 0, 0);
    surface.damage_buffer(0, 0, 16, 16);
    client.presentation.feedback(&surface).set_key(2u32);
    surface.commit();
    client.until("frame presented on bound output", |recorded| {
        recorded.synced.contains(&(2, bound))
    });
}

#[test]
fn fifo_commits_advance_on_refresh() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let manager: WpFifoManagerV1 = client.global(1);

    let window = client.toplevel(1);
    let surface = window.surface.clone();
    client.ack(&window.xdg, window.tag);
    let fifo = manager.get_fifo(&surface);
    let pixels = client.pixels(16, 16, 1);
    pixels.fill(0..16, [0, 0, 255, 255]);
    client.show(&surface, &pixels);

    for tag in 10..16u32 {
        surface.attach(Some(&pixels.buffer), 0, 0);
        surface.damage_buffer(0, 0, 16, 16);
        fifo.set_barrier();
        fifo.wait_barrier();
        if tag == 15 {
            surface.frame().set_key(tag);
        }
        surface.commit();
    }
    client.until("last queued commit presented", |recorded| {
        recorded.frames.contains(&15)
    });
}

#[test]
fn blur_region_tracks_backdrop_changes() {
    let harness = Harness::full(EXTENT, &APPEARANCE);
    let mut client = harness.client();
    let compositor: WlCompositor = client.global(6);
    let effects: ExtBackgroundEffectManagerV1 = client.global(1);

    let (behind, _behind_layer) = whole_layer(&mut client, 1);
    let backdrop = client.pixels(320, 240, 1);
    backdrop.fill(0..EDGE as i32, RED);
    backdrop.fill(EDGE as i32..240, BLUE);
    client.show(&behind, &backdrop);
    harness.frame_where(move |snapshot| {
        shows(snapshot, INSIDE, EDGE - 2, SHOWN_RED)
            && shows(snapshot, INSIDE, EDGE + 1, SHOWN_BLUE)
    });

    let (pane, _pane_layer) = whole_layer(&mut client, 2);
    let effect = effects.get_background_effect(&pane);
    let region = compositor.create_region();
    region.add(120, 60, 120, 120);
    effect.set_blur_region(Some(&region));
    let clear = client.pixels(320, 240, 2);
    clear.fill(0..240, CLEAR);
    client.show(&pane, &clear);

    let frosted = harness.frame_where(move |snapshot| blended(snapshot, INSIDE, EDGE));
    assert!(
        shows(&frosted, OUTSIDE, EDGE - 2, SHOWN_RED)
            && shows(&frosted, OUTSIDE, EDGE + 1, SHOWN_BLUE),
        "only the blur region is blurred"
    );

    backdrop.fill(0..240, GREEN);
    behind.attach(Some(&backdrop.buffer), 0, 0);
    behind.damage_buffer(0, 0, 320, 240);
    behind.commit();
    client.roundtrip();
    harness.frame_where(move |snapshot| {
        shows(snapshot, INSIDE, EDGE, SHOWN_GREEN) && shows(snapshot, OUTSIDE, EDGE, SHOWN_GREEN)
    });
}

#[test]
fn blur_settles_on_last_change() {
    let harness = Harness::full(EXTENT, &APPEARANCE);
    let mut client = harness.client();
    let compositor: WlCompositor = client.global(6);
    let effects: ExtBackgroundEffectManagerV1 = client.global(1);

    let (behind, _behind_layer) = whole_layer(&mut client, 1);
    let backdrop = client.pixels(320, 240, 1);
    backdrop.fill(0..240, RED);
    client.show(&behind, &backdrop);

    let (pane, _pane_layer) = whole_layer(&mut client, 2);
    let effect = effects.get_background_effect(&pane);
    let region = compositor.create_region();
    region.add(120, 60, 120, 120);
    effect.set_blur_region(Some(&region));
    let clear = client.pixels(320, 240, 2);
    clear.fill(0..240, CLEAR);
    client.show(&pane, &clear);
    harness.frame_where(move |snapshot| shows(snapshot, INSIDE, EDGE, SHOWN_RED));

    for color in [GREEN, RED, GREEN, BLUE] {
        backdrop.fill(0..240, color);
        behind.attach(Some(&backdrop.buffer), 0, 0);
        behind.damage_buffer(0, 0, 320, 240);
        behind.commit();
        client.roundtrip();
    }
    harness.frame_where(move |snapshot| {
        shows(snapshot, OUTSIDE, EDGE, SHOWN_BLUE) && shows(snapshot, INSIDE, EDGE, SHOWN_BLUE)
    });
}

#[test]
fn scrim_blurs_while_glass_blur_is_off() {
    let harness = Harness::full(EXTENT, &UNBLURRED);
    let mut client = harness.client();
    let compositor: WlCompositor = client.global(6);
    let effects: ExtBackgroundEffectManagerV1 = client.global(1);

    let (behind, _behind_layer) = whole_layer(&mut client, 1);
    let backdrop = client.pixels(320, 240, 1);
    backdrop.fill(0..EDGE as i32, RED);
    backdrop.fill(EDGE as i32..240, BLUE);
    client.show(&behind, &backdrop);
    harness.frame_where(move |snapshot| {
        shows(snapshot, OUTSIDE, EDGE - 2, SHOWN_RED)
            && shows(snapshot, OUTSIDE, EDGE + 1, SHOWN_BLUE)
    });

    let clear = client.pixels(320, 240, 2);
    clear.fill(0..240, CLEAR);
    let (pane, _pane_layer) = whole_layer(&mut client, 2);
    let glass = effects.get_background_effect(&pane);
    let left = compositor.create_region();
    left.add(0, 60, 100, 120);
    glass.set_blur_region(Some(&left));
    client.show(&pane, &clear);

    let (scrim, _scrim_layer) = client.scrim(3);
    let frost = effects.get_background_effect(&scrim);
    let right = compositor.create_region();
    right.add(120, 60, 120, 120);
    frost.set_blur_region(Some(&right));
    let veil = client.pixels(320, 240, 3);
    veil.fill(0..240, CLEAR);
    client.show(&scrim, &veil);

    let frosted = harness.frame_where(move |snapshot| blended(snapshot, INSIDE, EDGE));
    assert!(
        shows(&frosted, OUTSIDE, EDGE - 2, SHOWN_RED)
            && shows(&frosted, OUTSIDE, EDGE + 1, SHOWN_BLUE),
        "ordinary glass stays unblurred"
    );
    assert!(
        shows(&frosted, INSIDE, 70, SHOWN_RED) && shows(&frosted, INSIDE, 170, SHOWN_BLUE),
        "the scrim blurs without darkening: {:?} {:?}",
        frosted.pixel(INSIDE, 70),
        frosted.pixel(INSIDE, 170)
    );
}

#[test]
fn scrim_glass_follows_its_alpha_multiplier() {
    let harness = Harness::full(EXTENT, &APPEARANCE);
    let mut client = harness.client();
    let compositor: WlCompositor = client.global(6);
    let effects: ExtBackgroundEffectManagerV1 = client.global(1);
    let alphas: WpAlphaModifierV1 = client.global(1);

    let (behind, _behind_layer) = whole_layer(&mut client, 1);
    let backdrop = client.pixels(320, 240, 1);
    backdrop.fill(0..EDGE as i32, RED);
    backdrop.fill(EDGE as i32..240, BLUE);
    client.show(&behind, &backdrop);
    let sharp = move |snapshot: &Snapshot| {
        shows(snapshot, INSIDE, EDGE - 2, SHOWN_RED)
            && shows(snapshot, INSIDE, EDGE + 1, SHOWN_BLUE)
    };
    harness.frame_where(sharp);

    let (scrim, _scrim_layer) = client.scrim(2);
    let alpha = alphas.get_surface(&scrim);
    alpha.set_multiplier(u32::MAX);
    let frost = effects.get_background_effect(&scrim);
    let region = compositor.create_region();
    region.add(0, 0, 320, 240);
    frost.set_blur_region(Some(&region));
    let veil = client.pixels(320, 240, 2);
    veil.fill(0..240, CLEAR);
    client.show(&scrim, &veil);
    harness.frame_where(move |snapshot| blended(snapshot, INSIDE, EDGE));

    alpha.set_multiplier(0);
    scrim.commit();
    client.roundtrip();
    harness.frame_where(sharp);
}
