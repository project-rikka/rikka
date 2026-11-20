use crate::common::{Server, Wayland};
use rikka_compositor_surface::{AccessError, BufferContent};
use rikka_compositor_types::Fourcc;
use rikka_wayland_client::wl_shm::Format;
use rustix::fs::ftruncate;

#[test]
fn truncated_pool_marks_buffer_invalid() {
    let server = Server::start();
    let mut client = server.connect();
    let compositor = client.compositor();
    let surface = client.surface(&compositor);
    let shm = client.canvas(64, 64, Format::Argb8888);
    surface.attach(Some(&shm.buffer), 0, 0);
    surface.commit();
    client.roundtrip();
    ftruncate(&shm.memory, 0).unwrap();

    let id = surface.id();
    let (first, second, truncated) = server.call(move |state| {
        let surface = state.surface(id);
        let Some(BufferContent::Shm(buffer)) = state.surfaces.content(surface) else {
            panic!("expected shm content");
        };
        let first =
            buffer.with_pixels(|pixels| pixels.iter().map(|byte| u64::from(*byte)).sum::<u64>());
        let second = buffer.with_pixels(|pixels| pixels.len());
        (first, second, buffer.truncated())
    });
    assert_eq!(first, Err(AccessError::Truncated));
    assert_eq!(second, Err(AccessError::Truncated));
    assert!(truncated);
    assert!(client.fails());
    server.expect("protocol error 2 on wl_buffer");

    let survivor = server.connect();
    let _ = survivor.compositor();
}

#[test]
fn replaced_buffer_released_when_unheld() {
    let server = Server::start();
    let mut client = server.connect();
    let compositor = client.compositor();
    let surface = client.surface(&compositor);
    let first = client.canvas(8, 8, Format::Argb8888);
    let second = client.canvas(8, 8, Format::Argb8888);
    let third = client.canvas(8, 8, Format::Argb8888);
    surface.attach(Some(&first.buffer), 0, 0);
    surface.commit();
    client.roundtrip();
    let id = surface.id();
    let hold = server.call(move |state| {
        let surface = state.surface(id);
        state.surfaces.hold(surface).unwrap()
    });

    surface.attach(Some(&second.buffer), 0, 0);
    surface.commit();
    client.roundtrip();
    assert_eq!(client.received.released, Vec::<u32>::new());

    server.call(move |state| state.surfaces.release(hold));
    client.roundtrip();
    assert_eq!(client.received.released, vec![first.buffer.id()]);

    server.call(move |state| {
        let surface = state.surface(id);
        state.surfaces.uploaded(surface);
        assert!(state.surfaces.get(surface).unwrap().mapped());
    });
    client.roundtrip();
    assert_eq!(
        client.received.released,
        vec![first.buffer.id(), second.buffer.id()]
    );

    surface.attach(Some(&third.buffer), 0, 0);
    surface.commit();
    surface.attach(None, 0, 0);
    surface.commit();
    client.roundtrip();
    assert_eq!(
        client.received.released,
        vec![first.buffer.id(), second.buffer.id(), third.buffer.id()]
    );
}

#[test]
fn grown_pool_serves_new_range() {
    let server = Server::start();
    let mut client = server.connect();
    let compositor = client.compositor();
    let surface = client.surface(&compositor);
    let shm = client.canvas(8, 8, Format::Argb8888);
    ftruncate(&shm.memory, 512).unwrap();
    rustix::io::pwrite(&shm.memory, &[9; 256], 256).unwrap();
    shm.pool.resize(512);
    let tail = shm.pool.create_buffer(256, 8, 8, 32, Format::Xrgb8888);
    surface.attach(Some(&tail), 0, 0);
    surface.commit();
    client.roundtrip();
    let id = surface.id();
    let sum = server.call(move |state| {
        let Some(BufferContent::Shm(buffer)) = state.surfaces.content(state.surface(id)) else {
            panic!("expected shm content");
        };
        assert_eq!(buffer.format(), Fourcc::XRGB8888);
        buffer
            .with_pixels(|pixels| pixels.iter().map(|byte| u32::from(*byte)).sum::<u32>())
            .unwrap()
    });
    assert_eq!(sum, 9 * 256);
}

#[test]
fn opaque_format_makes_surface_opaque() {
    let server = Server::start();
    let mut client = server.connect();
    let compositor = client.compositor();
    let surface = client.surface(&compositor);
    let translucent = client.canvas(8, 8, Format::Argb8888);
    let solid = translucent
        .pool
        .create_buffer(0, 8, 8, 32, Format::Xrgb8888);
    let id = surface.id();
    let opaque = |client: &mut Wayland| {
        client.roundtrip();
        server.call(move |state| {
            let surface = state.surface(id);
            let opaque = state.surfaces.get(surface).unwrap().opaque();
            opaque.iter().collect::<Vec<_>>()
        })
    };

    surface.attach(Some(&solid), 0, 0);
    surface.commit();
    assert_eq!(
        opaque(&mut client),
        vec![rikka_compositor_geometry::Rect::from_size((8, 8).into())]
    );

    surface.attach(Some(&translucent.buffer), 0, 0);
    surface.commit();
    assert_eq!(opaque(&mut client), vec![]);
}
