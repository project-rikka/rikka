use crate::common::{Server, Wayland};
use rikka_compositor_surface::unblock;
use rikka_wayland_client::wl_shm::Format;
use rikka_wayland_client::wl_surface;
use rikka_wayland_client::{wp_viewport, wp_viewporter};

fn viewported(client: &Wayland) -> (wl_surface::WlSurface, wp_viewport::WpViewport) {
    let compositor = client.compositor();
    let surface = client.surface(&compositor);
    let viewporter: wp_viewporter::WpViewporter = client.global(1);
    let viewport = viewporter.get_viewport(&surface);
    (surface, viewport)
}

#[test]
fn source_outside_buffer_is_out_of_buffer() {
    let server = Server::start();
    let mut client = server.connect();
    let (surface, viewport) = viewported(&client);
    let shm = client.canvas(20, 20, Format::Argb8888);
    viewport.set_source(10.0, 10.0, 15.0, 5.0);
    surface.attach(Some(&shm.buffer), 0, 0);
    surface.commit();
    assert!(client.fails());
    server.expect("protocol error 2 on wp_viewport");
}

#[test]
fn source_checked_against_pending_commits() {
    let server = Server::start();
    let mut client = server.connect();
    let (surface, viewport) = viewported(&client);
    let large = client.canvas(20, 20, Format::Argb8888);
    viewport.set_source(0.0, 0.0, 20.0, 20.0);
    surface.attach(Some(&large.buffer), 0, 0);
    surface.commit();
    client.roundtrip();
    let id = surface.id();
    let blocker = server.call(move |state| state.gate(id));

    let small = client.canvas(10, 10, Format::Argb8888);
    viewport.set_source(0.0, 0.0, 10.0, 10.0);
    surface.attach(Some(&small.buffer), 0, 0);
    surface.commit();
    let again = client.canvas(10, 10, Format::Argb8888);
    surface.attach(Some(&again.buffer), 0, 0);
    surface.commit();
    client.roundtrip();

    let applied = server.call(move |state| {
        unblock(state, blocker);
        let surface = state.surface(id);
        (
            state.commits_of(surface).len(),
            state.surfaces.get(surface).unwrap().size(),
        )
    });
    assert_eq!(
        applied,
        (3, Some(rikka_compositor_geometry::Size::new(10, 10)))
    );
}

#[test]
fn short_stride_is_invalid() {
    let server = Server::start();
    let mut client = server.connect();
    let shm = client.canvas(16, 16, Format::Argb8888);
    shm.pool.create_buffer(0, 16, 16, 32, Format::Argb8888);
    assert!(client.fails());
    server.expect("protocol error 1 on wl_shm_pool");
}

#[test]
fn shrinking_pool_is_invalid() {
    let server = Server::start();
    let mut client = server.connect();
    let shm = client.canvas(16, 16, Format::Argb8888);
    shm.pool.resize(16);
    assert!(client.fails());
    server.expect("protocol error 1 on wl_shm_pool");
}
