use std::io::Read;
use std::os::fd::AsFd;

use rikka_nested::{Claim, Event, Input};
use rikka_types::{DragAction, Point, PointerButton};
use rikka_ui::{Delivery, DragOffer};
use rikka_wayland_client::wl_data_device_manager::DndAction;
use rustix::pipe::{PipeFlags, pipe_with};

use crate::support::{self, Client, roundtrip_until};

const TEXT: &str = "text/plain;charset=utf-8";

struct Carried(&'static str);

impl Delivery for Carried {
    fn read(&self, _mime: &str) -> std::io::Result<Vec<u8>> {
        Ok(self.0.as_bytes().to_vec())
    }
}

#[test]
fn host_drop_reaches_client_under_pointer() {
    let (nested, _events) = support::start();
    let view = nested.view(Claim::app_id("rikka-view-drop"));
    let mut client = Client::connect(nested.socket());
    client.follow(&view);
    let _device = client.data_device();
    let window = client.window(Some("rikka-view-drop"));
    client.map(&window, 60, 60);
    roundtrip_until(&mut client, |_| view.frame().layers.len() == 1);

    let offer = DragOffer::new([TEXT.to_owned()]).delivered_by(Carried("dropped words"));
    let at = Point::new(20.0, 20.0);
    view.deliver(Input::DragMoved {
        at,
        offer: offer.clone(),
    });
    roundtrip_until(&mut client, |client| {
        client
            .received
            .drag
            .iter()
            .any(|line| line.starts_with("enter"))
    });
    let entered = client
        .received
        .drag
        .iter()
        .find(|line| line.starts_with("enter"))
        .unwrap()
        .clone();
    let mut parts = entered.split(' ').skip(1);
    let id: u32 = parts.next().unwrap().parse().unwrap();
    let serial: u32 = parts.next().unwrap().parse().unwrap();
    assert_eq!(client.received.offers[&id], [TEXT]);
    let data_offer = client.received.data_offers[&id].clone();
    data_offer.accept(serial, Some(TEXT));
    data_offer.set_actions(DndAction::COPY, DndAction::COPY);
    roundtrip_until(&mut client, |_| view.look().drop_accepted);

    view.deliver(Input::DragDropped {
        at,
        offer,
        action: DragAction::Copy,
    });
    roundtrip_until(&mut client, |client| {
        client.received.drag.iter().any(|line| line == "drop")
    });
    let (read, write) = pipe_with(PipeFlags::CLOEXEC).unwrap();
    data_offer.receive(TEXT, write.as_fd());
    drop(write);
    client.roundtrip();
    let mut text = String::new();
    std::fs::File::from(read).read_to_string(&mut text).unwrap();
    assert_eq!(text, "dropped words");
    data_offer.finish();
    client.roundtrip();
}

#[test]
fn client_drag_offers_data_to_host() {
    let (nested, events) = support::start();
    let view = nested.view(Claim::app_id("rikka-view-drag"));
    let mut client = Client::connect(nested.socket());
    client.follow(&view);
    let _pointer = client.pointer();
    let device = client.data_device();
    let window = client.window(Some("rikka-view-drag"));
    client.map(&window, 60, 60);
    roundtrip_until(&mut client, |_| view.frame().layers.len() == 1);
    view.deliver(Input::Button {
        at: Point::new(10.0, 10.0),
        button: PointerButton::Primary,
        pressed: true,
    });
    roundtrip_until(&mut client, |client| {
        client
            .received
            .pointer
            .iter()
            .any(|line| line == "button 0x110 true")
    });
    let source = client.source(&[TEXT]);
    source.set_actions(DndAction::COPY | DndAction::MOVE);
    device.start_drag(
        Some(&source),
        &window.surface,
        None,
        client.received.pointer_serial,
    );
    roundtrip_until(&mut client, |_| {
        view.look()
            .drag
            .as_ref()
            .and_then(|drag| drag.offer.as_ref())
            .and_then(|offer| offer.data(TEXT))
            .is_some_and(|data| &*data == format!("nested {TEXT}").as_bytes())
    });
    let started = Event::DragStarted {
        view: view.id(),
        mime_types: vec![TEXT.to_owned()],
    };
    support::await_event(&events, &started);
}
