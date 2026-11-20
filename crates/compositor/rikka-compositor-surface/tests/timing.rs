use std::time::Duration;

use crate::common::Server;
use rikka_compositor_surface::{BufferChange, unblock};
use rikka_wayland_client::wl_shm::Format;

#[test]
fn frame_callbacks_fire_once_per_tree() {
    let server = Server::start();
    let mut client = server.connect();
    let compositor = client.compositor();
    let subcompositor: rikka_wayland_client::wl_subcompositor::WlSubcompositor = client.global(1);
    let parent = client.surface(&compositor);
    let child = client.surface(&compositor);
    let role = subcompositor.get_subsurface(&child, &parent);
    role.set_desync();
    let parent_buffer = client.canvas(16, 16, Format::Argb8888);
    let child_buffer = client.canvas(8, 8, Format::Argb8888);
    parent.attach(Some(&parent_buffer.buffer), 0, 0);
    parent
        .frame()
        .with_key(crate::common::client::Callback::Frame(1));
    parent.commit();
    child.attach(Some(&child_buffer.buffer), 0, 0);
    child
        .frame()
        .with_key(crate::common::client::Callback::Frame(2));
    child.commit();
    client.roundtrip();
    assert!(client.received.callbacks.is_empty());

    let id = parent.id();
    server.call(move |state| {
        let root = state.surface(id);
        assert!(state.surfaces.get(root).unwrap().awaits_frame());
        state.surfaces.frame_done(root, Duration::from_millis(1234));
        state.surfaces.frame_done(root, Duration::from_millis(2000));
    });
    client.roundtrip();
    let mut fired = client.received.callbacks.clone();
    fired.sort_by_key(|(kind, _)| format!("{kind:?}"));
    assert_eq!(
        fired,
        vec![
            (crate::common::client::Callback::Frame(1), 1234),
            (crate::common::client::Callback::Frame(2), 1234)
        ]
    );
}

#[test]
fn blocked_commit_holds_later_commits() {
    let server = Server::start();
    let mut client = server.connect();
    let compositor = client.compositor();
    let surface = client.surface(&compositor);
    let other = client.surface(&compositor);
    client.roundtrip();
    let id = surface.id();
    let blocker = server.call(move |state| state.gate(id));

    let first = client.canvas(8, 8, Format::Argb8888);
    let second = client.canvas(16, 16, Format::Argb8888);
    surface.attach(Some(&first.buffer), 0, 0);
    surface.commit();
    surface.attach(Some(&second.buffer), 0, 0);
    surface.commit();
    other.commit();
    client.roundtrip();
    let other_id = other.id();
    let early = server.call(move |state| {
        (
            state.commits_of(state.surface(id)).len(),
            state.commits_of(state.surface(other_id)).len(),
        )
    });
    assert_eq!(early, (0, 1));

    let late = server.call(move |state| {
        unblock(state, blocker);
        let surface = state.surface(id);
        (
            state
                .commits_of(surface)
                .iter()
                .map(|commit| (commit.buffer, commit.sequence))
                .collect::<Vec<_>>(),
            state.surfaces.get(surface).unwrap().size(),
        )
    });
    assert_eq!(
        late.0,
        vec![(BufferChange::Attached, 1), (BufferChange::Attached, 2)]
    );
    assert_eq!(late.1, Some(rikka_compositor_geometry::Size::new(16, 16)));
    client.roundtrip();
    assert_eq!(client.received.released, vec![first.buffer.id()]);
}

#[test]
fn commit_flood_disconnects_client() {
    let server = Server::start();
    let mut client = server.connect();
    let compositor = client.compositor();
    let surface = client.surface(&compositor);
    client.roundtrip();
    let id = surface.id();
    server.call(move |state| {
        state.gate(id);
    });
    for _ in 0..300 {
        surface.commit();
    }
    assert!(client.fails());
    server.expect("too many commits wait on one surface");
}
