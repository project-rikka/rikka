use crate::common::Server;
use rikka_compositor_geometry::{Buffer, Logical, Point, Rect, Size};
use rikka_compositor_surface::{BufferChange, BufferContent, Changes};
use rikka_compositor_types::Fourcc;
use rikka_wayland_client::wl_output::Transform;
use rikka_wayland_client::wl_shm::Format;
use rikka_wayland_client::wp_viewporter;

#[test]
fn committed_shm_buffer_reports_size_and_damage() {
    let server = Server::start();
    let mut client = server.connect();
    let compositor = client.compositor();
    let surface = client.surface(&compositor);
    let shm = client.canvas(64, 32, Format::Argb8888);
    shm.fill(0..32, [0, 1, 2, 3]);
    surface.attach(Some(&shm.buffer), 0, 0);
    surface.damage_buffer(8, 4, 16, 8);
    surface.commit();
    client.roundtrip();

    let id = surface.id();
    let (commits, size, buffer_size, damage, first_row) = server.call(move |state| {
        let surface = state.surface(id);
        let commits = state.commits_of(surface);
        let entry = state.surfaces.get(surface).unwrap();
        let size = entry.size();
        let buffer_size = entry.buffer_size();
        let damage = state.take_damage(surface);
        let first_row = match state.surfaces.content(surface) {
            Some(BufferContent::Shm(shm)) => {
                assert_eq!(shm.format(), Fourcc::ARGB8888);
                assert_eq!(shm.stride(), 256);
                shm.with_pixels(|pixels| pixels[..4].to_vec()).unwrap()
            }
            other => panic!("expected shm content, got {other:?}"),
        };
        (
            commits,
            size,
            buffer_size,
            (
                damage.buffer.iter().collect::<Vec<_>>(),
                damage.surface.iter().collect::<Vec<_>>(),
            ),
            first_row,
        )
    });
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].buffer, BufferChange::Attached);
    assert!(commits[0].changes.contains(Changes::SIZE | Changes::DAMAGE));
    assert_eq!(size, Some(Size::<i32, Logical>::new(64, 32)));
    assert_eq!(buffer_size, Some(Size::<i32, Buffer>::new(64, 32)));
    assert_eq!(damage.0, vec![Rect::from_size(Size::new(64, 32))]);
    assert_eq!(damage.1, vec![Rect::from_size(Size::new(64, 32))]);
    assert_eq!(first_row, vec![0, 1, 2, 3]);

    surface.damage(2, 2, 4, 4);
    surface.commit();
    client.roundtrip();
    let damage = server.call(move |state| {
        let surface = state.surface(id);
        let damage = state.take_damage(surface);
        (
            damage.buffer.iter().collect::<Vec<_>>(),
            damage.surface.iter().collect::<Vec<_>>(),
            state.commits_of(surface).last().unwrap().buffer,
        )
    });
    assert_eq!(damage.0, vec![rect(2, 2, 4, 4)]);
    assert_eq!(damage.1, vec![rect(2, 2, 4, 4)]);
    assert_eq!(damage.2, BufferChange::Unchanged);
}

#[test]
fn damage_maps_through_scale_and_transform() {
    let server = Server::start();
    let mut client = server.connect();
    let compositor = client.compositor();
    let surface = client.surface(&compositor);
    let shm = client.canvas(200, 100, Format::Argb8888);
    surface.set_buffer_scale(2);
    surface.attach(Some(&shm.buffer), 0, 0);
    surface.commit();
    client.roundtrip();
    let id = surface.id();
    let size = server.call(move |state| {
        let surface = state.surface(id);
        let _ = state.take_damage(surface);
        state.surfaces.get(surface).unwrap().size()
    });
    assert_eq!(size, Some(Size::new(100, 50)));

    surface.damage(5, 5, 10, 10);
    surface.damage_buffer(100, 0, 20, 20);
    surface.commit();
    client.roundtrip();
    let (buffer, logical) = server.call(move |state| {
        let surface = state.surface(id);
        let damage = state.take_damage(surface);
        let mut buffer: Vec<_> = damage.buffer.iter().collect();
        let mut logical: Vec<_> = damage.surface.iter().collect();
        buffer.sort_by_key(|rect| rect.loc.x);
        logical.sort_by_key(|rect| rect.loc.x);
        (buffer, logical)
    });
    assert_eq!(buffer, vec![rect(10, 10, 20, 20), rect(100, 0, 20, 20)]);
    assert_eq!(logical, vec![rect(5, 5, 10, 10), rect(50, 0, 10, 10)]);

    surface.set_buffer_transform(Transform::_90);
    surface.commit();
    client.roundtrip();
    let (size, changes) = server.call(move |state| {
        let surface = state.surface(id);
        (
            state.surfaces.get(surface).unwrap().size(),
            state.commits_of(surface).last().unwrap().changes,
        )
    });
    assert_eq!(size, Some(Size::new(50, 100)));
    assert!(changes.contains(Changes::TRANSFORM | Changes::SIZE | Changes::DAMAGE));
}

#[test]
fn viewport_sets_size_and_source() {
    let server = Server::start();
    let mut client = server.connect();
    let compositor = client.compositor();
    let surface = client.surface(&compositor);
    let viewporter: wp_viewporter::WpViewporter = client.global(1);
    let viewport = viewporter.get_viewport(&surface);
    let shm = client.canvas(100, 100, Format::Argb8888);
    surface.set_buffer_scale(2);
    viewport.set_source(10.0, 10.0, 20.0, 30.0);
    surface.attach(Some(&shm.buffer), 0, 0);
    surface.commit();
    client.roundtrip();
    let id = surface.id();
    let (size, source) = server.call(move |state| {
        let entry = state.surfaces.get(state.surface(id)).unwrap();
        (entry.size(), entry.source_in_buffer())
    });
    assert_eq!(size, Some(Size::new(20, 30)));
    assert_eq!(
        source,
        Some(Rect::new(Point::new(20.0, 20.0), Size::new(40.0, 60.0)))
    );

    viewport.set_destination(40, 60);
    surface.commit();
    viewport.destroy();
    surface.commit();
    client.roundtrip();
    let size = server.call(move |state| state.surfaces.get(state.surface(id)).unwrap().size());
    assert_eq!(size, Some(Size::new(50, 50)));
}

fn rect<S>(x: i32, y: i32, w: i32, h: i32) -> Rect<i32, S> {
    Rect::new(Point::new(x, y), Size::new(w, h))
}
