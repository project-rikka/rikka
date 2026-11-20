use rikka_nested::Event;
use rikka_types::Size;

use crate::support::{self, Client, roundtrip_until};

#[test]
fn named_toplevel_attaches_to_new_view() {
    let (nested, events) = support::start();
    let mut client = Client::connect(nested.socket());
    let window = client.window(Some("rikka-view-1"));
    client.map(&window, 64, 48);
    let view = nested.view("rikka-view-1");
    client.follow(&view);
    roundtrip_until(&mut client, |_| view.look().attached);
    support::await_event(&events, &Event::Attached { view: view.id() });
    roundtrip_until(&mut client, |_| view.frame().layers.len() == 1);
    assert_eq!(view.frame().size, Size::new(64.0, 48.0));
}

#[test]
fn late_app_id_attaches_view() {
    let (nested, _events) = support::start();
    let view = nested.view("rikka-view-7");
    view.resize(Size::new(320.0, 200.0), 1.0);
    let mut client = Client::connect(nested.socket());
    client.follow(&view);
    let window = client.window(None);
    client.map(&window, 100, 80);
    assert!(!view.look().attached);
    window.toplevel.set_app_id("rikka-view-7");
    window.surface.commit();
    roundtrip_until(&mut client, |client| {
        client
            .received
            .configures
            .last()
            .is_some_and(|(width, height, _)| (*width, *height) == (320, 200))
    });
    assert!(view.look().attached);
}

#[test]
fn destroyed_toplevel_empties_view() {
    let (nested, events) = support::start();
    let view = nested.view("rikka-view-2");
    let mut client = Client::connect(nested.socket());
    client.follow(&view);
    let window = client.window(Some("rikka-view-2"));
    client.map(&window, 40, 40);
    roundtrip_until(&mut client, |_| !view.frame().layers.is_empty());
    window.toplevel.destroy();
    window.xdg.destroy();
    window.surface.destroy();
    roundtrip_until(&mut client, |_| !view.look().attached);
    assert!(view.frame().layers.is_empty());
    support::await_event(&events, &Event::Detached { view: view.id() });
}

#[test]
fn titles_reach_view() {
    let (nested, _events) = support::start();
    let view = nested.view("rikka-view-3");
    let mut client = Client::connect(nested.socket());
    client.follow(&view);
    let window = client.window(Some("rikka-view-3"));
    window.toplevel.set_title("Example Domain");
    client.map(&window, 40, 40);
    roundtrip_until(&mut client, |_| {
        view.look().title.as_deref() == Some("Example Domain")
    });
}
