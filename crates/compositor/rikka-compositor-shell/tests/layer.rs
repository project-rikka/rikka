use crate::common::Server;
use rikka_compositor_geometry::Size;
use rikka_compositor_protocols::zwlr_layer_shell_v1::Layer;
use rikka_compositor_protocols::zwlr_layer_surface_v1::{Anchor, KeyboardInteractivity};
use rikka_compositor_shell::Margins;
use rikka_wayland_client::wl_shm::Format;
use rikka_wayland_client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

fn layer_shell(client: &crate::common::Wayland) -> zwlr_layer_shell_v1::ZwlrLayerShellV1 {
    client.global(5)
}

#[test]
fn layer_surface_lifecycle() {
    let server = Server::start();
    let mut client = server.connect();
    let shell = layer_shell(&client);
    let surface = client.surface();
    let layer = shell
        .get_layer_surface(&surface, None, zwlr_layer_shell_v1::Layer::Top, "bar")
        .with_key(1u32);
    layer.set_size(0, 30);
    layer.set_anchor(
        zwlr_layer_surface_v1::Anchor::TOP
            | zwlr_layer_surface_v1::Anchor::LEFT
            | zwlr_layer_surface_v1::Anchor::RIGHT,
    );
    layer.set_exclusive_zone(30);
    layer.set_margin(1, 2, 3, 4);
    layer.set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::OnDemand);
    surface.commit();
    client.roundtrip();
    server.expect("event: Created");
    server.expect("event: InitialCommit");

    let id = server.call(|state| state.last_layer());
    let (namespace, state) = server.call(move |state| {
        let layer = state.shell.layer(id).unwrap();
        (layer.namespace().to_owned(), *layer.state())
    });
    assert_eq!(namespace, "bar");
    assert_eq!(state.layer, Layer::Top);
    assert_eq!(state.anchor, Anchor::TOP | Anchor::LEFT | Anchor::RIGHT);
    assert_eq!(state.exclusive_zone, 30);
    assert_eq!(
        state.margins,
        Margins {
            top: 1,
            right: 2,
            bottom: 3,
            left: 4
        }
    );
    assert_eq!(
        state.keyboard_interactivity,
        KeyboardInteractivity::OnDemand
    );
    assert_eq!(state.size, Size::new(0, 30));

    let serial = server
        .call(move |state| state.shell.configure_layer(id, Size::new(1920, 30)))
        .unwrap();
    client.roundtrip();
    assert_eq!(
        client.received.layer_configures,
        vec![(1, serial, 1920, 30)]
    );
    layer.ack_configure(serial);
    let buffer = client.canvas(1920, 30, Format::Argb8888);
    surface.attach(Some(&buffer.buffer), 0, 0);
    surface.commit();
    client.roundtrip();
    server.expect("event: Configured");
    server.expect("event: Mapped");
    let size = server.call(move |state| state.shell.layer(id).unwrap().size());
    assert_eq!(size, Some(Size::new(1920, 30)));

    layer.set_exclusive_zone(40);
    surface.commit();
    client.roundtrip();
    server.expect("event: Changed");

    surface.attach(None, 0, 0);
    surface.commit();
    client.roundtrip();
    server.expect("event: Unmapped");
    let (mapped, zone) = server.call(move |state| {
        let layer = state.shell.layer(id).unwrap();
        (layer.mapped(), layer.state().exclusive_zone)
    });
    assert!(!mapped);
    assert_eq!(zone, 0);

    server.call(move |state| state.shell.close_layer(id));
    client.roundtrip();
    assert_eq!(client.received.layers_closed, vec![1]);
    layer.destroy();
    client.roundtrip();
    server.expect("event: Destroyed");
}

#[test]
fn unconfigured_layer_buffer_is_invalid_state() {
    let server = Server::start();
    let mut client = server.connect();
    let shell = layer_shell(&client);
    let surface = client.surface();
    let layer = shell
        .get_layer_surface(&surface, None, zwlr_layer_shell_v1::Layer::Bottom, "dock")
        .with_key(1u32);
    layer.set_size(10, 10);
    let buffer = client.canvas(10, 10, Format::Argb8888);
    surface.attach(Some(&buffer.buffer), 0, 0);
    surface.commit();
    assert!(client.fails());
    server.expect("protocol error 0 on zwlr_layer_surface_v1");
}

#[test]
fn remap_requires_fresh_configure() {
    let server = Server::start();
    let mut client = server.connect();
    let shell = layer_shell(&client);
    let surface = client.surface();
    let layer = shell
        .get_layer_surface(&surface, None, zwlr_layer_shell_v1::Layer::Overlay, "ime")
        .with_key(1u32);
    layer.set_size(200, 80);
    surface.commit();
    client.roundtrip();
    let id = server.call(|state| state.last_layer());
    let first = server
        .call(move |state| state.shell.configure_layer(id, Size::new(200, 80)))
        .unwrap();
    client.roundtrip();
    layer.ack_configure(first);
    let buffer = client.canvas(200, 80, Format::Argb8888);
    surface.attach(Some(&buffer.buffer), 0, 0);
    surface.commit();
    client.roundtrip();
    server.expect("event: Mapped");

    let late = server
        .call(move |state| state.shell.configure_layer(id, Size::new(240, 80)))
        .unwrap();
    surface.attach(None, 0, 0);
    surface.commit();
    client.roundtrip();
    server.expect("event: Unmapped");
    layer.ack_configure(late);
    client.roundtrip();

    layer.set_size(240, 80);
    surface.commit();
    client.roundtrip();
    server.expect("event: InitialCommit");
    let again = server
        .call(move |state| state.shell.configure_layer(id, Size::new(240, 80)))
        .unwrap();
    client.roundtrip();
    layer.ack_configure(again);
    let buffer = client.canvas(240, 80, Format::Argb8888);
    surface.attach(Some(&buffer.buffer), 0, 0);
    surface.commit();
    client.roundtrip();
    server.expect("event: Mapped");

    surface.attach(None, 0, 0);
    surface.commit();
    client.roundtrip();
    server.expect("event: Unmapped");
    layer.set_size(240, 80);
    surface.attach(Some(&buffer.buffer), 0, 0);
    surface.commit();
    assert!(client.fails());
    server.expect("protocol error 0 on zwlr_layer_surface_v1");
}
