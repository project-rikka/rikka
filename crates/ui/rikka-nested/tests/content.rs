use std::time::{Duration, Instant};

use rikka_nested::{Claim, LayerContent};
use rikka_picture::Tick;
use rikka_types::Size;

use crate::support::{self, Client, roundtrip_until};

#[test]
fn shm_buffer_copied_and_released() {
    let (nested, _events) = support::start();
    let view = nested.view(Claim::app_id("rikka-view-shm"));
    let mut client = Client::connect(nested.socket());
    client.follow(&view);
    let window = client.window(Some("rikka-view-shm"));
    let buffer = client.map(&window, 30, 20);
    roundtrip_until(&mut client, |_| view.frame().layers.len() == 1);
    let frame = view.frame();
    let layer = &frame.layers[0];
    assert_eq!(layer.rect.size, Size::new(30.0, 20.0));
    let LayerContent::Feed(feed) = &layer.content else {
        panic!("shared memory arrives as a feed, not {:?}", layer.content);
    };
    assert_eq!(feed.size(), Some(Size::new(30.0, 20.0)));
    let id = buffer.id();
    roundtrip_until(&mut client, |client| client.received.released.contains(&id));
}

#[test]
fn dmabuf_released_after_hold_ends() {
    let (nested, _events) = support::start();
    let view = nested.view(Claim::app_id("rikka-view-dmabuf"));
    let mut client = Client::connect(nested.socket());
    client.follow(&view);
    let window = client.window(Some("rikka-view-dmabuf"));
    client.ack(&window.xdg);
    let first = client.dmabuf_buffer(16, 16);
    client.show(&window.surface, &first);
    roundtrip_until(&mut client, |_| view.frame().layers.len() == 1);
    let frame = view.frame();
    let LayerContent::Imported(imported) = &frame.layers[0].content else {
        panic!(
            "a dmabuf arrives imported, not {:?}",
            frame.layers[0].content
        );
    };
    assert_eq!(imported.size(), Some(Size::new(16.0, 16.0)));
    let first_id = first.id();
    client.roundtrip();
    assert!(!client.received.released.contains(&first_id));

    let second = client.dmabuf_buffer(16, 16);
    client.show(&window.surface, &second);
    roundtrip_until(&mut client, |client| {
        client.received.released.contains(&first_id)
    });
    assert!(!client.received.released.contains(&second.id()));
}

#[test]
fn frame_callbacks_follow_presentation() {
    let (nested, _events) = support::start();
    let view = nested.view(Claim::app_id("rikka-view-frames"));
    view.resize(Size::new(30.0, 20.0), 1.0);
    let mut client = Client::connect(nested.socket());
    client.follow(&view);
    let window = client.window(Some("rikka-view-frames"));
    client.map(&window, 30, 20);
    roundtrip_until(&mut client, |_| view.frame().layers.len() == 1);
    let before = client.received.frames;
    window.surface.frame();
    window.surface.commit();
    client.roundtrip();
    view.pulse().beat(Tick {
        at: Instant::now(),
        period: Duration::from_millis(5),
    });
    roundtrip_until(&mut client, |client| client.received.frames > before);
}
