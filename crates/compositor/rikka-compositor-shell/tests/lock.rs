use crate::common::{Server, Wayland};
use rikka_compositor_geometry::Size;
use rikka_wayland_client::wl_shm::Format;
use rikka_wayland_client::{
    ext_session_lock_manager_v1, ext_session_lock_surface_v1, ext_session_lock_v1, wl_output,
    wl_surface,
};

struct Locker {
    lock: ext_session_lock_v1::ExtSessionLockV1,
    output: wl_output::WlOutput,
}

fn lock(client: &mut Wayland, tag: u32) -> Locker {
    let manager: ext_session_lock_manager_v1::ExtSessionLockManagerV1 = client.global(1);
    let output: wl_output::WlOutput = client.global(4);
    let lock = manager.lock().with_key(tag);
    client.roundtrip();
    Locker { lock, output }
}

fn lock_surface(
    client: &Wayland,
    locker: &Locker,
) -> (
    wl_surface::WlSurface,
    ext_session_lock_surface_v1::ExtSessionLockSurfaceV1,
) {
    let surface = client.surface();
    let lock_surface = locker.lock.get_lock_surface(&surface, &locker.output);
    (surface, lock_surface)
}

#[test]
fn lock_confirms_and_unlocks() {
    let server = Server::start();
    let mut client = server.connect();
    let locker = lock(&mut client, 1);
    server.expect("Lock(Requested(");
    let lock_id = server.call(|state| state.locks[0]);
    assert!(server.call(move |state| state.shell.lock_session(lock_id)));
    client.roundtrip();
    assert_eq!(client.received.locked, vec![1]);
    assert!(server.call(|state| state.shell.session_locked()));

    let (surface, lock_surface) = lock_surface(&client, &locker);
    client.roundtrip();
    server.expect("Lock(SurfaceCreated(");
    let id = server.call(|state| state.lock_surfaces[0]);
    let serial = server
        .call(move |state| state.shell.configure_lock_surface(id, Size::new(64, 48)))
        .unwrap();
    client.roundtrip();
    assert_eq!(client.received.lock_configures, vec![(serial, 64, 48)]);
    lock_surface.ack_configure(serial);
    let buffer = client.canvas(64, 48, Format::Argb8888);
    surface.attach(Some(&buffer.buffer), 0, 0);
    surface.commit();
    client.roundtrip();
    let size = server.call(move |state| state.shell.lock_surface(id).unwrap().size());
    assert_eq!(size, Some(Size::new(64, 48)));

    locker.lock.unlock_and_destroy();
    client.roundtrip();
    server.expect("Lock(Unlocked(");
    assert!(!server.call(|state| state.shell.session_locked()));
}

#[test]
fn second_locker_is_finished() {
    let server = Server::start();
    let mut first = server.connect();
    let _held = lock(&mut first, 1);
    let mut second = server.connect();
    let _refused = lock(&mut second, 2);
    assert_eq!(second.received.finished, vec![2]);
    assert!(first.received.finished.is_empty());
    let requested = server.call(|state| state.locks.len());
    assert_eq!(requested, 1);
}

#[test]
fn dead_locker_keeps_session_locked() {
    let server = Server::start();
    let mut client = server.connect();
    let locker = lock(&mut client, 1);
    let lock_id = server.call(|state| state.locks[0]);
    server.call(move |state| state.shell.lock_session(lock_id));
    client.roundtrip();
    drop(locker);
    drop(client);
    server.expect("Lock(LockerGone(");
    assert!(server.call(|state| state.shell.session_locked()));

    let mut rescuer = server.connect();
    let _rescue = lock(&mut rescuer, 2);
    server.expect("Lock(Requested(");
    let rescue = server.call(|state| state.locks[1]);
    assert!(server.call(move |state| state.shell.lock_session(rescue)));
    rescuer.roundtrip();
    assert_eq!(rescuer.received.locked, vec![2]);
}

#[test]
fn wrong_size_is_dimensions_mismatch() {
    let server = Server::start();
    let mut client = server.connect();
    let locker = lock(&mut client, 1);
    let (surface, lock_surface) = lock_surface(&client, &locker);
    client.roundtrip();
    let id = server.call(|state| state.lock_surfaces[0]);
    let serial = server
        .call(move |state| state.shell.configure_lock_surface(id, Size::new(64, 48)))
        .unwrap();
    client.roundtrip();
    lock_surface.ack_configure(serial);
    let buffer = client.canvas(32, 48, Format::Argb8888);
    surface.attach(Some(&buffer.buffer), 0, 0);
    surface.commit();
    assert!(client.fails());
    server.expect("protocol error 2 on ext_session_lock_surface_v1");
}
