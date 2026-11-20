use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::sync::mpsc;
use std::thread::JoinHandle;

use rikka_compositor::{Compositor, Journal, Launch, install_logging};
use rikka_compositor_reactor::Sender;
use rikka_session::profile::Profile;
use tempfile::TempDir;

use crate::common::{Client, PATIENCE, mapped};

const SOCKET: &str = "wayland-secondary-gpu";

const VSYNC: u32 = 0x1;
const HW_CLOCK: u32 = 0x2;
const HW_COMPLETION: u32 = 0x4;

type Task = Box<dyn FnOnce(&mut Compositor) + Send>;

struct Session {
    tasks: Sender<Task>,
    thread: Option<JoinHandle<()>>,
    runtime: TempDir,
    _config: TempDir,
}

impl Session {
    fn on_displays() -> Self {
        assert!(
            Path::new("/sys/devices/faux/vkms").exists()
                || Path::new("/sys/devices/platform/vkms").exists(),
            "vkms device present (load with `sudo modprobe vkms`)"
        );
        let runtime = tempfile::tempdir().unwrap();
        std::fs::set_permissions(runtime.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let config = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("LIBSEAT_BACKEND", "noop");
            std::env::set_var("XDG_RUNTIME_DIR", runtime.path());
            std::env::set_var("XDG_CONFIG_HOME", config.path());
            std::env::remove_var("WAYLAND_DISPLAY");
            std::env::remove_var("DISPLAY");
            std::env::remove_var("DBUS_SESSION_BUS_ADDRESS");
        }
        let _logging = install_logging(Journal::Headless);
        let (ready, started) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("rikka-test-compositor".into())
            .spawn(move || {
                let mut compositor = Launch::kms()
                    .profile(Profile::Simple)
                    .desktop(false)
                    .socket(SOCKET)
                    .start()
                    .expect("compositor drives reachable outputs");
                let tasks = compositor
                    .reactor()
                    .mailbox(|compositor: &mut Compositor, task: Task| task(compositor))
                    .unwrap();
                ready.send(tasks).unwrap();
                compositor.run().unwrap();
            })
            .unwrap();
        let tasks = started.recv_timeout(PATIENCE).expect("compositor started");
        Self {
            tasks,
            thread: Some(thread),
            runtime,
            _config: config,
        }
    }

    fn client(&self) -> Client {
        Client::connect(&self.runtime.path().join(SOCKET))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let stopping: Task = Box::new(Compositor::stop);
        if self.tasks.send(stopping).is_ok()
            && let Some(thread) = self.thread.take()
        {
            let _ = thread.join();
        }
    }
}

#[test]
fn secondary_gpu_output_presents_at_vblank() {
    let session = Session::on_displays();
    let mut client = session.client();
    let output = client.output();
    client.until("virtual output described", |recorded| {
        recorded
            .outputs
            .iter()
            .any(|described| described.done && described.mode.0 > 0 && described.mode.1 > 0)
    });

    let window = mapped(&mut client, 1, (64, 48), [0x20, 0x80, 0xe0, 0xff]);
    let pixels = client.pixels(64, 48, 2);
    pixels.fill(0..48, [0xe0, 0x80, 0x20, 0xff]);
    window.surface.frame().set_key(2u32);
    client.presentation.feedback(&window.surface).set_key(2u32);
    client.show(&window.surface, &pixels);

    client.until("frame presented by display device", |recorded| {
        recorded
            .presented
            .iter()
            .any(|presented| presented.tag == 2)
    });
    let presented = client
        .received
        .presented
        .iter()
        .find(|presented| presented.tag == 2)
        .copied()
        .unwrap();
    assert_eq!(
        presented.flags & (VSYNC | HW_CLOCK | HW_COMPLETION),
        VSYNC | HW_CLOCK | HW_COMPLETION,
        "frame flipped by display device: {presented:?}"
    );
    assert!(
        client.received.synced.contains(&(2, output.id())),
        "frame reached virtual output: {:?}",
        client.received.synced
    );
    assert!(client.received.discarded.is_empty());
}
