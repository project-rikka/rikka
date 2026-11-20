use rikka_compositor::Extent;
use rikka_seam::Request;
use rikka_session::toplevels::{CAPABILITY, STATE, State};
use rikka_types::{Rect, RoundedRect, WindowView};
use rikka_wayland::Host;
use rikka_wayland_client::ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1;
use rikka_wayland_client::wl_compositor::WlCompositor;
use rikka_wayland_client::zwlr_layer_surface_v1::Anchor;
use rikka_window_view::WindowViews;

use crate::common::{Harness, shows};

const EXTENT: Extent = Extent {
    width: 320,
    height: 240,
};

const APPEARANCE: [(&str, &str); 1] = [("rikka.appearance", "blur = true\nblur-radius = 24.0\n")];

const EDGE: i32 = 120;

const CLEAR: [u8; 4] = [0, 0, 0, 0];
const RED: [u8; 4] = [0, 0, 255, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [255, 0, 0, 255];
const SHOWN_RED: [u8; 4] = [255, 0, 0, 255];
const SHOWN_GREEN: [u8; 4] = [0, 255, 0, 255];
const SHOWN_BLUE: [u8; 4] = [0, 0, 255, 255];

#[test]
fn window_view_draws_inside_clip() {
    let harness = Harness::full(EXTENT, &[]);
    let mut client = harness.client();
    let window = client.toplevel(1);
    client.ack(&window.xdg, 1);
    let content = client.pixels(40, 30, 1);
    content.fill(0..30, RED);
    client.show(&window.surface, &content);

    let (panel, _layer) = client.layer(2, Anchor::BOTTOM | Anchor::RIGHT, (120, 80), 0);
    let backing = client.pixels(120, 80, 2);
    backing.fill(0..80, BLUE);
    client.show(&panel, &backing);
    harness.frame_where(move |snapshot| shows(snapshot, 270, 200, SHOWN_BLUE));

    let state: State = harness.ask(Request::new(CAPABILITY, STATE));
    let id = state.toplevels[0].id.clone();
    let host = Host::read_by_owner(client.display());
    let views = WindowViews::bind(&host).expect("window view global");
    let mut shown = views.inside(&panel);
    shown.show(&[WindowView::new(id, Rect::new(10.0, 10.0, 80.0, 60.0))
        .clip(RoundedRect::rect(Rect::new(0.0, 0.0, 50.0, 80.0)))]);
    panel.commit();
    client.roundtrip();
    harness.frame_where(move |snapshot| {
        shows(snapshot, 230, 200, SHOWN_RED) && shows(snapshot, 270, 200, SHOWN_BLUE)
    });

    content.fill(0..30, GREEN);
    client.show(&window.surface, &content);
    harness.frame_where(move |snapshot| shows(snapshot, 230, 200, SHOWN_GREEN));

    shown.show(&[]);
    panel.commit();
    client.roundtrip();
    harness.frame_where(move |snapshot| shows(snapshot, 230, 200, SHOWN_BLUE));
}

#[test]
fn window_view_keeps_blur() {
    let harness = Harness::full(EXTENT, &APPEARANCE);
    let mut client = harness.client();
    let compositor: WlCompositor = client.global(6);
    let effects: ExtBackgroundEffectManagerV1 = client.global(1);

    let window = client.toplevel(1);
    client.ack(&window.xdg, 1);
    let effect = effects.get_background_effect(&window.surface);
    let region = compositor.create_region();
    region.add(0, 0, 40, 40);
    effect.set_blur_region(Some(&region));
    let clear = client.pixels(40, 40, 1);
    clear.fill(0..40, CLEAR);
    client.show(&window.surface, &clear);

    let (panel, _layer) = client.layer(2, Anchor::all(), (0, 0), -1);
    let backing = client.pixels(320, 240, 2);
    backing.fill(0..EDGE, RED);
    backing.fill(EDGE..240, BLUE);
    client.show(&panel, &backing);
    harness.frame_where(move |snapshot| shows(snapshot, 140, EDGE as u32 - 2, SHOWN_RED));

    let state: State = harness.ask(Request::new(CAPABILITY, STATE));
    let id = state.toplevels[0].id.clone();
    let host = Host::read_by_owner(client.display());
    let views = WindowViews::bind(&host).expect("window view global");
    let mut shown = views.inside(&panel);
    let placed = Rect::new(100.3, 90.3, 80.0, 60.0);
    shown.show(&[WindowView::new(id, placed).clip(RoundedRect::rect(placed))]);
    panel.commit();
    client.roundtrip();
    harness.frame_where(move |snapshot| {
        snapshot
            .pixel(140, EDGE as u32)
            .is_some_and(|[red, _, blue, _]| red > 40 && blue > 40)
    });
}
