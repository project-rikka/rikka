use std::fs::{File, OpenOptions};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

use crate::common::{Server, State, Wayland};
use drm::control::{Device as ControlDevice, syncobj};
use rikka_compositor_surface::{DmabufFeedback, DmabufTranche, TrancheFlags};
use rikka_compositor_types::{Format, Fourcc, Modifier};
use rikka_wayland_client::wl_shm;
use rikka_wayland_client::{wl_buffer, wl_surface};
use rikka_wayland_client::{
    wp_linux_drm_syncobj_manager_v1, wp_linux_drm_syncobj_surface_v1,
    wp_linux_drm_syncobj_timeline_v1,
};
use rikka_wayland_client::{zwp_linux_buffer_params_v1, zwp_linux_dmabuf_v1};
use rustix::fs::{MemfdFlags, ftruncate, memfd_create};

const RENDER_NODE: &str = "/dev/dri/renderD128";

struct Card(File);

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl drm::Device for Card {}

impl ControlDevice for Card {}

fn card() -> Card {
    Card(
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(RENDER_NODE)
            .unwrap(),
    )
}

fn enabled(state: &mut State) {
    let display = state.display.clone();
    let feedback = DmabufFeedback::new(
        0,
        vec![DmabufTranche {
            device: 0,
            flags: TrancheFlags::SAMPLING,
            formats: vec![Format::new(Fourcc::ARGB8888, Modifier::LINEAR)],
        }],
    )
    .unwrap();
    state.surfaces.enable_dmabuf(&display, feedback);
    let device: OwnedFd = card().0.into();
    state.surfaces.enable_syncobj(&display, device).unwrap();
}

struct Scene {
    client: Wayland,
    surface: wl_surface::WlSurface,
    sync: wp_linux_drm_syncobj_surface_v1::WpLinuxDrmSyncobjSurfaceV1,
    manager: wp_linux_drm_syncobj_manager_v1::WpLinuxDrmSyncobjManagerV1,
    dmabuf: zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1,
    memory: OwnedFd,
}

impl Scene {
    fn new(server: &Server) -> Self {
        let client = server.connect();
        let compositor = client.compositor();
        let surface = client.surface(&compositor);
        let manager: wp_linux_drm_syncobj_manager_v1::WpLinuxDrmSyncobjManagerV1 = client.global(1);
        let sync = manager.get_surface(&surface);
        let dmabuf: zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1 = client.global(6);
        let memory = memfd_create("syncobj-test", MemfdFlags::CLOEXEC).unwrap();
        ftruncate(&memory, 64 * 16).unwrap();
        Self {
            client,
            surface,
            sync,
            manager,
            dmabuf,
            memory,
        }
    }

    fn buffer(&self) -> wl_buffer::WlBuffer {
        let params = self.dmabuf.create_params();
        params.add(self.memory.as_fd(), 0, 0, 64, 0, 0);
        params.create_immed(
            16,
            16,
            Fourcc::ARGB8888.0,
            zwp_linux_buffer_params_v1::Flags::empty(),
        )
    }

    fn timeline(
        &self,
        card: &Card,
    ) -> (
        syncobj::Handle,
        wp_linux_drm_syncobj_timeline_v1::WpLinuxDrmSyncobjTimelineV1,
    ) {
        let handle = card.create_syncobj(false).unwrap();
        let fd = card.syncobj_to_fd(handle, false).unwrap();
        let timeline = self.manager.import_timeline(fd.as_fd());
        (handle, timeline)
    }
}

fn reached(card: &Card, handle: syncobj::Handle) -> u64 {
    let mut points = [0];
    card.syncobj_timeline_query(&[handle], &mut points, false)
        .unwrap();
    points[0]
}

#[test]
fn acquire_points_block_and_release_signals() {
    let server = Server::with(enabled);
    let mut scene = Scene::new(&server);
    let card = card();
    let (handle, timeline) = scene.timeline(&card);
    let first = scene.buffer();
    scene.sync.set_acquire_point(&timeline, 0, 1);
    scene.sync.set_release_point(&timeline, 0, 2);
    scene.surface.attach(Some(&first), 0, 0);
    scene.surface.commit();
    scene.client.roundtrip();
    let id = scene.surface.id();
    let applied = server.call(move |state| state.commits_of(state.surface(id)).len());
    assert_eq!(applied, 0);

    card.syncobj_timeline_signal(&[handle], &[1]).unwrap();
    server.await_commits(id, 1);
    assert_eq!(reached(&card, handle), 1);

    server.call(move |state| {
        let surface = state.surface(id);
        state.surfaces.uploaded(surface);
    });
    assert_eq!(reached(&card, handle), 2);
}

fn rejected(code: u32, act: impl FnOnce(&Scene, &Card)) {
    let server = Server::with(enabled);
    let mut scene = Scene::new(&server);
    let card = card();
    act(&scene, &card);
    assert!(scene.client.fails());
    server.expect(&format!(
        "protocol error {code} on wp_linux_drm_syncobj_surface_v1"
    ));
}

#[test]
fn explicit_sync_errors_are_protocol_errors() {
    rejected(3, |scene, card| {
        let (_, timeline) = scene.timeline(card);
        scene.sync.set_acquire_point(&timeline, 0, 1);
        scene.surface.commit();
    });
    rejected(2, |scene, card| {
        let (_, timeline) = scene.timeline(card);
        let shm = scene.client.canvas(8, 8, wl_shm::Format::Argb8888);
        scene.sync.set_acquire_point(&timeline, 0, 1);
        scene.sync.set_release_point(&timeline, 0, 2);
        scene.surface.attach(Some(&shm.buffer), 0, 0);
        scene.surface.commit();
    });
    rejected(4, |scene, card| {
        let (_, timeline) = scene.timeline(card);
        scene.sync.set_release_point(&timeline, 0, 2);
        scene.surface.attach(Some(&scene.buffer()), 0, 0);
        scene.surface.commit();
    });
    rejected(5, |scene, card| {
        let (_, timeline) = scene.timeline(card);
        scene.sync.set_acquire_point(&timeline, 0, 1);
        scene.surface.attach(Some(&scene.buffer()), 0, 0);
        scene.surface.commit();
    });
    rejected(6, |scene, card| {
        let (_, timeline) = scene.timeline(card);
        scene.sync.set_acquire_point(&timeline, 0, 2);
        scene.sync.set_release_point(&timeline, 0, 2);
        scene.surface.attach(Some(&scene.buffer()), 0, 0);
        scene.surface.commit();
    });
    rejected(1, |scene, card| {
        let (_, timeline) = scene.timeline(card);
        scene.surface.destroy();
        scene.sync.set_acquire_point(&timeline, 0, 1);
    });
}
