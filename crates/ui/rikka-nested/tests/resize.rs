use std::time::{Duration, Instant};

use rikka_nested::{Input, View};
use rikka_types::Size;

use crate::support::{self, Client, Window, roundtrip_until};

const MAXIMIZED: u32 = 1;
const FULLSCREEN: u32 = 2;
const ACTIVATED: u32 = 4;
const TILED: [u32; 4] = [5, 6, 7, 8];
const SUSPENDED: u32 = 9;

const RESIZE_TIMEOUT_BOUND: Duration = Duration::from_secs(2);

fn shown(client: &mut Client, view: &View, width: f32, height: f32) {
    roundtrip_until(client, |_| {
        view.frame()
            .layers
            .first()
            .is_some_and(|layer| layer.rect.size == Size::new(width, height))
    });
}

fn configured(client: &mut Client, width: i32, height: i32) -> Vec<u32> {
    roundtrip_until(client, |client| {
        client
            .received
            .configures
            .last()
            .is_some_and(|(w, h, _)| (*w, *h) == (width, height))
    });
    client.received.configures.last().unwrap().2.clone()
}

fn mapped(nested: &rikka_nested::Nested, app_id: &str) -> (View, Client, Window) {
    let view = nested.view(app_id);
    view.resize(Size::new(100.0, 80.0), 1.0);
    let mut client = Client::connect(nested.socket());
    client.follow(&view);
    let window = client.window(Some(app_id));
    configured(&mut client, 100, 80);
    client.map(&window, 100, 80);
    shown(&mut client, &view, 100.0, 80.0);
    (view, client, window)
}

#[test]
fn view_configures_toplevel_states() {
    let (nested, _events) = support::start();
    let (view, mut client, _window) = mapped(&nested, "rikka-view-states");
    let states = client.received.configures.last().unwrap().2.clone();
    assert!(states.contains(&MAXIMIZED));
    assert!(TILED.iter().all(|tiled| states.contains(tiled)));
    assert!(!states.contains(&ACTIVATED));

    view.deliver(Input::Focus(true));
    roundtrip_until(&mut client, |client| {
        client
            .received
            .configures
            .last()
            .unwrap()
            .2
            .contains(&ACTIVATED)
    });
    view.set_visible(false);
    roundtrip_until(&mut client, |client| {
        client
            .received
            .configures
            .last()
            .unwrap()
            .2
            .contains(&SUSPENDED)
    });
    view.set_fullscreen(true);
    roundtrip_until(&mut client, |client| {
        client
            .received
            .configures
            .last()
            .unwrap()
            .2
            .contains(&FULLSCREEN)
    });
}

#[test]
fn resize_keeps_old_frame_until_answered() {
    let (nested, _events) = support::start();
    let (view, mut client, window) = mapped(&nested, "rikka-view-resize");
    let before = view.frame().generation;

    view.resize(Size::new(200.0, 120.0), 1.0);
    configured(&mut client, 200, 120);
    let stale = client.shm_buffer(110, 85, 0xff11_2233);
    client.show(&window.surface, &stale);
    client.barrier();
    assert_eq!(view.frame().generation, before);
    assert_eq!(view.frame().layers[0].rect.size, Size::new(100.0, 80.0));

    client.ack(&window.xdg);
    let fresh = client.shm_buffer(200, 120, 0xff11_2233);
    client.show(&window.surface, &fresh);
    shown(&mut client, &view, 200.0, 120.0);
}

#[test]
fn unanswered_resize_times_out() {
    let (nested, _events) = support::start();
    let (view, mut client, window) = mapped(&nested, "rikka-view-stuck");
    view.resize(Size::new(300.0, 200.0), 1.0);
    configured(&mut client, 300, 200);
    let started = Instant::now();
    let stale = client.shm_buffer(120, 90, 0xff44_5566);
    client.show(&window.surface, &stale);
    shown(&mut client, &view, 120.0, 90.0);
    assert!(started.elapsed() < RESIZE_TIMEOUT_BOUND);
}
