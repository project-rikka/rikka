use crate::common::{Server, Wayland};
use rikka_compositor_geometry::{Point, Rect, Size};
use rikka_compositor_output::blur_region;
use rikka_wayland_client::{ext_background_effect_manager_v1, wl_surface};

type Manager = ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1;

fn blur_of(
    server: &Server,
    surface: &wl_surface::WlSurface,
) -> Option<Vec<Rect<i32, rikka_compositor_geometry::Logical>>> {
    let protocol_id = surface.id();
    server.call(move |state| {
        let id = state.surface(protocol_id);
        blur_region(&state.surfaces, id).map(|region| region.iter().collect())
    })
}

fn manager(client: &mut Wayland) -> Manager {
    let manager: Manager = client.global(1);
    client.roundtrip();
    manager
}

#[test]
fn blur_region_applies_on_commit() {
    let server = Server::start();
    let mut client = server.connect();
    let manager = manager(&mut client);
    let surface = client.surface();
    let effect = manager.get_background_effect(&surface);
    let region = client.region(&[(0, 0, 10, 10)]);
    effect.set_blur_region(Some(&region));
    client.roundtrip();
    assert_eq!(blur_of(&server, &surface), None);

    surface.commit();
    client.roundtrip();
    let square = Rect::new(Point::new(0, 0), Size::new(10, 10));
    assert_eq!(blur_of(&server, &surface), Some(vec![square]));
    assert_eq!(server.call(|state| state.blur_commits.len()), 1);

    let wider = client.region(&[(0, 0, 20, 10)]);
    effect.set_blur_region(Some(&wider));
    client.roundtrip();
    assert_eq!(blur_of(&server, &surface), Some(vec![square]));

    surface.commit();
    client.roundtrip();
    assert_eq!(
        blur_of(&server, &surface),
        Some(vec![Rect::new(Point::new(0, 0), Size::new(20, 10))])
    );

    surface.commit();
    client.roundtrip();
    assert_eq!(server.call(|state| state.blur_commits.len()), 2);
}

#[test]
fn second_effect_object_is_protocol_error() {
    let server = Server::start();
    let mut client = server.connect();
    let manager = manager(&mut client);
    let surface = client.surface();
    let _first = manager.get_background_effect(&surface);
    let _second = manager.get_background_effect(&surface);
    assert!(client.fails());
}
