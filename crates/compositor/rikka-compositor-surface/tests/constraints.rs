use std::time::{Duration, Instant};

use crate::common::{Server, Wayland, output};
use rikka_compositor_surface::refreshed;
use rikka_test_display::Canvas;
use rikka_wayland_client::wl_shm::Format;
use rikka_wayland_client::wl_surface;
use rikka_wayland_client::wp_commit_timing_manager_v1;
use rikka_wayland_client::wp_fifo_manager_v1;
use rustix::time::{ClockId, clock_gettime};

const TARGET_DELAY: Duration = Duration::from_millis(60);

const EARLY_TOLERANCE: Duration = Duration::from_millis(10);

fn mapped(client: &Wayland) -> (wl_surface::WlSurface, Canvas) {
    let compositor = client.compositor();
    let surface = client.surface(&compositor);
    let shm = client.canvas(8, 8, Format::Argb8888);
    surface.attach(Some(&shm.buffer), 0, 0);
    (surface, shm)
}

fn applied(server: &Server, id: u32) -> usize {
    server.call(move |state| state.commits_of(state.surface(id)).len())
}

fn now() -> Duration {
    let now = clock_gettime(ClockId::Monotonic);
    Duration::new(now.tv_sec as u64, now.tv_nsec as u32)
}

#[test]
fn fifo_barrier_waits_for_refresh() {
    let server = Server::start();
    let mut client = server.connect();
    let (surface, _shm) = mapped(&client);
    let manager: wp_fifo_manager_v1::WpFifoManagerV1 = client.global(1);
    let fifo = manager.get_fifo(&surface);
    client.roundtrip();
    let id = surface.id();
    server.call(move |state| {
        let surface = state.surface(id);
        state.surfaces.set_main_output(surface, Some(output(3)));
    });

    fifo.set_barrier();
    surface.commit();
    fifo.wait_barrier();
    surface.commit();
    client.roundtrip();
    assert_eq!(applied(&server, id), 1);
    let refresh_requested = server.call(|state| state.refresh_wanted.contains(&output(3)));
    assert!(refresh_requested);

    server.call(|state| refreshed(state, output(9), Duration::ZERO));
    assert_eq!(applied(&server, id), 1);
    server.call(|state| refreshed(state, output(3), Duration::ZERO));
    assert_eq!(applied(&server, id), 2);

    fifo.wait_barrier();
    surface.commit();
    client.roundtrip();
    assert_eq!(applied(&server, id), 3);
}

#[test]
fn timed_commit_waits_for_target() {
    let server = Server::start();
    let mut client = server.connect();
    let (surface, _shm) = mapped(&client);
    let manager: wp_commit_timing_manager_v1::WpCommitTimingManagerV1 = client.global(1);
    let timer = manager.get_timer(&surface);
    let target = now() + TARGET_DELAY;
    let started = Instant::now();
    timer.set_timestamp(
        (target.as_secs() >> 32) as u32,
        target.as_secs() as u32,
        target.subsec_nanos(),
    );
    surface.commit();
    client.roundtrip();
    let id = surface.id();
    assert_eq!(applied(&server, id), 0);
    server.await_commits(id, 1);
    assert!(started.elapsed() >= TARGET_DELAY - EARLY_TOLERANCE);
}
