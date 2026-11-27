use rikka_nested::{Claim, Event, Input};
use rikka_types::Size;

use crate::support::{self, Client, roundtrip_until};

const MAXIMIZED: u32 = 1;

#[test]
fn named_toplevel_attaches_to_new_view() {
    let (nested, events) = support::start();
    let mut client = Client::connect(nested.socket());
    let window = client.window(Some("rikka-view-1"));
    client.map(&window, 64, 48);
    let view = nested.view(Claim::app_id("rikka-view-1"));
    client.follow(&view);
    roundtrip_until(&mut client, |_| view.look().attached);
    support::await_event(&events, &Event::Attached { view: view.id() });
    roundtrip_until(&mut client, |_| view.frame().layers.len() == 1);
    assert_eq!(view.frame().size, Size::new(64.0, 48.0));
}

#[test]
fn late_app_id_attaches_view() {
    let (nested, _events) = support::start();
    let view = nested.view(Claim::app_id("rikka-view-7"));
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
    let view = nested.view(Claim::app_id("rikka-view-2"));
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
    let view = nested.view(Claim::app_id("rikka-view-3"));
    let mut client = Client::connect(nested.socket());
    client.follow(&view);
    let window = client.window(Some("rikka-view-3"));
    window.toplevel.set_title("Example Domain");
    client.map(&window, 40, 40);
    roundtrip_until(&mut client, |_| {
        view.look().title.as_deref() == Some("Example Domain")
    });
}

#[test]
fn title_token_claims_toplevel_and_keeps_it_after_retitle() {
    let (nested, events) = support::start();
    let mut client = Client::connect(nested.socket());
    let other = client.window(Some("google-chrome"));
    other.toplevel.set_title("Inbox - Google Chrome");
    client.map(&other, 30, 30);
    let window = client.window(Some("google-chrome"));
    window.toplevel.set_title("rikka-claim-5 - Google Chrome");
    client.map(&window, 50, 40);
    let view = nested.view(Claim::title("rikka-claim-5"));
    client.follow(&view);
    roundtrip_until(&mut client, |_| view.frame().size == Size::new(50.0, 40.0));
    support::await_event(&events, &Event::Attached { view: view.id() });
    window.toplevel.set_title("Example Domain - Google Chrome");
    window.surface.commit();
    roundtrip_until(&mut client, |_| {
        view.look().title.as_deref() == Some("Example Domain - Google Chrome")
    });
    assert!(view.look().attached);
}

#[test]
fn unclaimed_toplevel_is_announced_and_adopted_by_key() {
    let (nested, events) = support::start();
    let mut client = Client::connect(nested.socket());
    let window = client.window(Some("google-chrome"));
    window.toplevel.set_title("Sign in - Google Chrome");
    client.map(&window, 60, 30);
    let toplevel = loop {
        match events.recv_timeout(support::TIMEOUT) {
            Ok(Event::Unclaimed {
                toplevel,
                title: Some(title),
            }) if title == "Sign in - Google Chrome" => break toplevel,
            Ok(_) => {}
            Err(_) => panic!("the titled toplevel was never announced"),
        }
    };
    let view = nested.view(Claim::toplevel(toplevel));
    client.follow(&view);
    roundtrip_until(&mut client, |_| view.frame().size == Size::new(60.0, 30.0));
    support::await_event(&events, &Event::UnclaimedClosed { toplevel });
}

#[test]
fn dependent_toplevel_shows_centered_over_its_owner() {
    let (nested, _events) = support::start();
    let view = nested.view(Claim::app_id("rikka-view-owner"));
    view.resize(Size::new(200.0, 100.0), 1.0);
    let mut client = Client::connect(nested.socket());
    client.follow(&view);
    let owner = client.window(Some("rikka-view-owner"));
    client.map(&owner, 200, 100);
    roundtrip_until(&mut client, |_| view.frame().layers.len() == 1);
    let dialog = client.window(None);
    dialog.toplevel.set_parent(Some(&owner.toplevel));
    dialog.surface.commit();
    client.roundtrip();
    client.map(&dialog, 80, 40);
    roundtrip_until(&mut client, |_| view.frame().layers.len() == 2);
    let shown = &view.frame().layers[1];
    assert_eq!(
        (shown.rect.origin.x, shown.rect.origin.y, shown.rect.size),
        (60.0, 30.0, Size::new(80.0, 40.0))
    );
    dialog.toplevel.destroy();
    dialog.xdg.destroy();
    dialog.surface.destroy();
    roundtrip_until(&mut client, |_| view.frame().layers.len() == 1);
}

#[test]
fn anonymous_toplevel_joins_the_focused_view_at_its_top_right() {
    let (nested, _events) = support::start();
    let view = nested.view(Claim::app_id("rikka-view-bubble-owner"));
    view.resize(Size::new(400.0, 300.0), 1.0);
    let mut client = Client::connect(nested.socket());
    client.follow(&view);
    let owner = client.window(Some("rikka-view-bubble-owner"));
    client.map(&owner, 400, 300);
    roundtrip_until(&mut client, |_| view.frame().layers.len() == 1);
    view.deliver(Input::Focus(true));
    let bubble = client.window(Some(""));
    client.map(&bubble, 100, 50);
    roundtrip_until(&mut client, |_| view.frame().layers.len() == 2);
    let shown = &view.frame().layers[1];
    assert_eq!(
        (shown.rect.origin.x, shown.rect.origin.y, shown.rect.size),
        (292.0, 8.0, Size::new(100.0, 50.0))
    );
    let (_, _, states) = client
        .received
        .configures
        .last()
        .expect("the bubble was configured");
    assert!(
        !states.contains(&MAXIMIZED),
        "the bubble is never maximized"
    );
}
