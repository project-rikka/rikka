use crate::common;

use std::sync::Arc;
use std::time::Instant;

use crate::common::{UNCOVERED, renderer};
use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::Size;
use rikka_ui::*;
use rikka_ui_host::ImageCache;
use rikka_widgets::*;

const VIEW: u32 = 64;

fn pixel(pixels: &[u8], x: usize, y: usize) -> (u8, u8, u8, u8) {
    common::pixel(pixels, VIEW as usize, x, y)
}

fn blue_pixels() -> Arc<[u8]> {
    Arc::from([0, 0, 255, 255].repeat(4))
}

#[derive(State)]
struct Wall {
    pixels: Arc<[u8]>,
}

fn wall_view(wall: &Wall) -> Element {
    image(ImageSource::Rgba {
        width: 2,
        height: 2,
        pixels: Arc::clone(&wall.pixels),
    })
    .width(Length::px(VIEW as f32))
    .height(Length::px(VIEW as f32))
    .build()
}

#[test]
fn committed_image_draws_then_stays_cached() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(VIEW, VIEW).unwrap();
    let mut scene = Scene::new();
    let mut cache = ImageCache::new();

    let mut ui = TypedUi::new(
        Wall {
            pixels: blue_pixels(),
        },
        wall_view,
        Theme::dark(),
    );
    ui.set_background(Some(UNCOVERED));
    ui.set_viewport(Size::new(VIEW as f32, VIEW as f32), 1.0);
    let now = Instant::now();

    ui.render(
        Rendering::new(&mut scene)
            .with_time(now)
            .resolving(&mut cache.resolver()),
    );
    renderer.draw(&mut scene, &mut target).unwrap();
    let px = renderer.read_pixels(&mut target).unwrap();
    assert_eq!(
        pixel(&px, 32, 32),
        (255, 0, 0, 255),
        "unresolved image drew over background"
    );
    assert!(cache.is_loading(), "pixels not pending upload");

    let mut settled_keys = Vec::new();
    let mut updated = Vec::new();
    cache.commit(&renderer, &mut settled_keys, &mut updated);
    assert_eq!(settled_keys.len(), 1, "upload not finished");
    assert!(!cache.is_loading());
    ui.request_frame();
    ui.render(
        Rendering::new(&mut scene)
            .with_time(now)
            .resolving(&mut cache.resolver()),
    );
    let out = renderer.draw(&mut scene, &mut target).unwrap();
    assert!(out.rendered());
    let px = renderer.read_pixels(&mut target).unwrap();
    assert_eq!(
        pixel(&px, 32, 32),
        (0, 0, 255, 255),
        "uploaded image does not cover background"
    );

    ui.update(|_| {});
    assert!(ui.needs_render());
    ui.render(
        Rendering::new(&mut scene)
            .with_time(now)
            .resolving(&mut cache.resolver()),
    );
    let out = renderer.draw(&mut scene, &mut target).unwrap();
    assert!(!out.rendered(), "unchanged image damaged scene");
}

#[test]
fn undecodable_image_settles() {
    let renderer = renderer();
    let mut cache = ImageCache::new();
    let broken = ImageSource::Bytes(Arc::from(b"not a picture at all".to_vec()));

    assert!(cache.resolver().resolve(&broken).is_none());

    let mut settled = Vec::new();
    let mut updated = Vec::new();
    while cache.is_loading() {
        cache.commit(&renderer, &mut settled, &mut updated);
    }

    assert_eq!(
        settled,
        vec![broken.key()],
        "broken image not reported as settled"
    );
    assert!(updated.is_empty());
}

#[test]
fn cache_evicts_off_screen_first() {
    let renderer = renderer();
    let mut cache = ImageCache::keeping(0);
    let shown = ImageSource::Rgba {
        width: 2,
        height: 2,
        pixels: blue_pixels(),
    };
    let put_away = ImageSource::Rgba {
        width: 2,
        height: 2,
        pixels: Arc::from([255, 0, 0, 255].repeat(4)),
    };

    let mut settled = Vec::new();
    let mut updated = Vec::new();
    for source in [&shown, &put_away] {
        assert!(cache.resolver().resolve(source).is_none());
    }
    cache.commit(&renderer, &mut settled, &mut updated);
    assert_eq!(settled.len(), 2, "both images uploaded");

    cache.shown(std::iter::once(&shown.key()));
    cache.commit(&renderer, &mut settled, &mut updated);

    assert!(
        cache.resolver().resolve(&shown).is_some(),
        "visible image evicted under budget"
    );
    assert!(
        cache.resolver().resolve(&put_away).is_none(),
        "undrawn image not evicted"
    );
}

#[test]
fn cache_releases_unreachable_images() {
    let renderer = renderer();
    let mut cache = ImageCache::keeping(1024);
    let held_source = ImageSource::Rgba {
        width: 2,
        height: 2,
        pixels: blue_pixels(),
    };
    let dropped_pixels = blue_pixels();
    let dropped = Arc::downgrade(&dropped_pixels);
    let dropped_source = ImageSource::Rgba {
        width: 2,
        height: 2,
        pixels: dropped_pixels,
    };

    let mut settled = Vec::new();
    let mut updated = Vec::new();
    for source in [&held_source, &dropped_source] {
        assert!(cache.resolver().resolve(source).is_none());
    }
    cache.commit(&renderer, &mut settled, &mut updated);
    assert_eq!(settled.len(), 2, "both images uploaded");
    settled.clear();

    drop(dropped_source);
    cache.shown(std::iter::empty());
    assert!(cache.has_work(), "unheld image requested no release commit");
    cache.commit(&renderer, &mut settled, &mut updated);

    assert!(
        dropped.upgrade().is_none(),
        "unreachable image pixels not freed"
    );
    assert!(
        cache.resolver().resolve(&held_source).is_some(),
        "held_source off-screen image evicted"
    );
}

#[test]
fn feed_released_after_drop() {
    let renderer = renderer();
    let mut cache = ImageCache::keeping(0);
    let feed = Feed::new();
    feed.show(2, 2, [0, 0, 255, 255].repeat(4));
    let source = ImageSource::Feed(feed.clone());

    let mut settled = Vec::new();
    let mut updated = Vec::new();
    assert!(cache.resolver().resolve(&source).is_none());
    cache.commit(&renderer, &mut settled, &mut updated);
    assert!(
        cache.resolver().resolve(&source).is_some(),
        "frame not uploaded"
    );

    cache.shown(std::iter::empty());
    cache.commit(&renderer, &mut settled, &mut updated);
    assert!(
        cache.resolver().resolve(&source).is_some(),
        "held_source feed keeps off-screen frame"
    );

    drop(source);
    drop(feed);
    cache.shown(std::iter::empty());
    cache.commit(&renderer, &mut settled, &mut updated);
    assert!(
        !cache.has_work(),
        "dropped feed released without further commits"
    );
}
