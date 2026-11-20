use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use rikka_compositor::{Compositor, Extent, Injected, Launch};
use rikka_compositor_reactor::Sender;
use rikka_seam::Request;
use rikka_seam::net::{SocketClient, socket_path_for};
use rikka_session::profile::Profile;
use serde::de::DeserializeOwned;
use tempfile::TempDir;

const SOCKET: &str = "wayland-frame-cost";

pub(crate) const COMPOSITOR_THREAD: &str = "rikka-bench-compositor";

const PATIENCE: Duration = Duration::from_secs(20);

const GLASS: &str =
    "theme = \"material-you-light\"\nbackground = \"liquid\"\nblur = true\nblur-radius = 48.0\n";

const SOLID: &str = "theme = \"light\"\nblur = false\n";

const WINDOWS: &str = "layout = \"floating\"\n";

type Task = Box<dyn FnOnce(&mut Compositor) + Send>;

pub(crate) struct Session {
    tasks: Option<Sender<Task>>,
    thread: Option<JoinHandle<()>>,
    worker: i32,
    socket: PathBuf,
    _runtime: TempDir,
    config: TempDir,
}

impl Session {
    pub(crate) fn start(extent: Extent, glass: bool, millihertz: u32) -> Self {
        let runtime = tempfile::tempdir().unwrap();
        std::fs::set_permissions(runtime.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let config = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("XDG_RUNTIME_DIR", runtime.path());
            std::env::set_var("XDG_CONFIG_HOME", config.path());
            std::env::set_var("XDG_STATE_HOME", config.path().join("state"));
            std::env::remove_var("WAYLAND_DISPLAY");
            std::env::remove_var("DISPLAY");
            std::env::remove_var("DBUS_SESSION_BUS_ADDRESS");
            std::env::remove_var(rikka_seam::net::SOCKET_ENV);
        }
        let written = config.path().join("rikka");
        std::fs::create_dir_all(&written).unwrap();
        let appearance = match glass {
            true => GLASS,
            false => SOLID,
        };
        for (domain, contents) in [("rikka.appearance", appearance), ("rikka.windows", WINDOWS)] {
            std::fs::write(written.join(format!("{domain}.toml")), contents).unwrap();
        }
        let (ready, started) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name(COMPOSITOR_THREAD.into())
            .spawn(move || {
                let mut compositor = Launch::headless_at(extent, millihertz)
                    .profile(Profile::Full)
                    .desktop(false)
                    .socket(SOCKET)
                    .start()
                    .expect("headless compositor starts on a Vulkan device");
                let tasks = compositor
                    .reactor()
                    .mailbox(|compositor: &mut Compositor, task: Task| task(compositor))
                    .unwrap();
                ready.send((tasks, unsafe { libc::gettid() })).unwrap();
                compositor.run().unwrap();
            })
            .unwrap();
        let (tasks, worker) = started.recv_timeout(PATIENCE).expect("compositor started");
        Self {
            tasks: Some(tasks),
            thread: Some(thread),
            worker,
            socket: runtime.path().join(SOCKET),
            _runtime: runtime,
            config,
        }
    }

    pub(crate) fn socket(&self) -> &std::path::Path {
        &self.socket
    }

    pub(crate) fn config(&self) -> &std::path::Path {
        self.config.path()
    }

    pub(crate) fn compositor_cpu(&self) -> Duration {
        let path = format!("/proc/self/task/{}/schedstat", self.worker);
        std::fs::read_to_string(path)
            .ok()
            .and_then(|stat| stat.split_whitespace().next()?.parse().ok())
            .map(Duration::from_nanos)
            .unwrap_or_default()
    }

    pub(crate) fn ask<T: DeserializeOwned>(&self, request: Request) -> T {
        let path = socket_path_for(SOCKET).expect("runtime directory set");
        let seam = SocketClient::connect(path).expect("session seam socket connects");
        seam.client()
            .call(request)
            .blocking_wait()
            .and_then(|payload| payload.decode())
            .expect("session seam reply")
    }

    pub(crate) fn inject(&self, injected: &[Injected]) {
        let injected = injected.to_vec();
        let (reply, answer) = mpsc::channel();
        self.tasks
            .as_ref()
            .unwrap()
            .send(Box::new(move |compositor| {
                for one in injected {
                    compositor.inject(one);
                }
                let _ = reply.send(());
            }))
            .ok()
            .unwrap();
        answer.recv_timeout(PATIENCE).unwrap();
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(tasks) = self.tasks.take() {
            let _ = tasks.send(Box::new(Compositor::stop));
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
