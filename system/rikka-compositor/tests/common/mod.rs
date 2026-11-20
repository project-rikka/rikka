pub(crate) mod capture;
pub(crate) mod client;
pub(crate) mod input;
pub(crate) mod shell;
pub(crate) mod shortcuts;

use rikka_sync::{Mutex, MutexGuard};
use std::cell::Cell;
use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use rikka_compositor::{Compositor, Extent, Injected, Launch, Snapshot};
use rikka_compositor_reactor::Sender;
use rikka_seam::Request;
use rikka_seam::net::{SocketClient, socket_path_for};
use rikka_session::profile::Profile;
use serde::de::DeserializeOwned;
use tempfile::TempDir;
use x11rb::rust_connection::RustConnection;

pub(crate) use client::{Client, Presented};
pub(crate) use shell::Toplevel;

pub(crate) const PATIENCE: Duration = Duration::from_secs(10);

pub(crate) const BTN_LEFT: u32 = 0x110;

const SOCKET: &str = "wayland-harness";

static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

type Task = Box<dyn FnOnce(&mut Compositor) + Send>;

pub(crate) struct Harness {
    tasks: Option<Sender<Task>>,
    thread: Option<JoinHandle<()>>,
    socket: PathBuf,
    _runtime: TempDir,
    _config: TempDir,
    state: TempDir,
    _shell: Option<FakeShell>,
    _alone: MutexGuard<'static, ()>,
}

struct FakeShell {
    _bin: TempDir,
    path: Option<OsString>,
}

impl Harness {
    pub(crate) fn headless(extent: Extent) -> Self {
        Self::start(extent, Profile::Simple, &[], false, None)
    }

    pub(crate) fn full(extent: Extent, settings: &[(&str, &str)]) -> Self {
        Self::start(extent, Profile::Full, settings, false, None)
    }

    pub(crate) fn starting_locked(extent: Extent, settings: &[(&str, &str)]) -> Self {
        Self::start(extent, Profile::Simple, settings, true, None)
    }

    pub(crate) fn supervising(extent: Extent, shell: &str) -> Self {
        Self::start(extent, Profile::Simple, &[], false, Some(shell))
    }

    fn start(
        extent: Extent,
        profile: Profile,
        settings: &[(&str, &str)],
        locked: bool,
        shell: Option<&str>,
    ) -> Self {
        let alone = ONE_AT_A_TIME.lock();
        let runtime = tempfile::tempdir().unwrap();
        std::fs::set_permissions(runtime.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let config = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let shell = shell.map(FakeShell::install);
        unsafe {
            std::env::set_var("XDG_RUNTIME_DIR", runtime.path());
            std::env::set_var("XDG_CONFIG_HOME", config.path());
            std::env::set_var("XDG_STATE_HOME", state.path());
            std::env::remove_var("WAYLAND_DISPLAY");
            std::env::remove_var("DISPLAY");
            std::env::remove_var("DBUS_SESSION_BUS_ADDRESS");
        }
        let written = config.path().join("rikka");
        std::fs::create_dir_all(&written).unwrap();
        for (domain, contents) in settings {
            std::fs::write(written.join(format!("{domain}.toml")), contents).unwrap();
        }

        let desktop = shell.is_some();
        let (ready, started) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("rikka-test-compositor".into())
            .spawn(move || {
                let mut compositor = Launch::headless(extent)
                    .profile(profile)
                    .desktop(desktop)
                    .locked(locked)
                    .socket(SOCKET)
                    .start()
                    .expect("headless compositor starts on a Vulkan device");
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
            tasks: Some(tasks),
            thread: Some(thread),
            socket: runtime.path().join(SOCKET),
            _runtime: runtime,
            _config: config,
            state,
            _shell: shell,
            _alone: alone,
        }
    }

    pub(crate) fn state(&self) -> &std::path::Path {
        self.state.path()
    }

    pub(crate) fn call<T: Send + 'static>(
        &self,
        task: impl FnOnce(&mut Compositor) -> T + Send + 'static,
    ) -> T {
        let (reply, answer) = mpsc::channel();
        self.tasks
            .as_ref()
            .unwrap()
            .send(Box::new(move |compositor| {
                let _ = reply.send(task(compositor));
            }))
            .ok()
            .unwrap();
        answer.recv_timeout(PATIENCE).unwrap()
    }

    pub(crate) fn socket(&self) -> &std::path::Path {
        &self.socket
    }

    pub(crate) fn client(&self) -> Client {
        Client::connect(&self.socket)
    }

    pub(crate) fn globals(&self) -> Vec<String> {
        interfaces(&rikka_test_display::Client::<()>::connect_to(
            &self.socket,
            |_| {},
        ))
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
        self.call(move |compositor| {
            for one in injected {
                compositor.inject(one);
            }
        });
    }

    pub(crate) fn click(&self, x: f64, y: f64) {
        self.inject(&[
            Injected::PointerTo { x, y },
            Injected::Button {
                code: BTN_LEFT,
                pressed: true,
            },
            Injected::Button {
                code: BTN_LEFT,
                pressed: false,
            },
        ]);
    }

    pub(crate) fn press(&self, keys: &[u32]) {
        let mut injected: Vec<Injected> = keys
            .iter()
            .map(|code| Injected::Key {
                code: *code,
                pressed: true,
            })
            .collect();
        injected.extend(keys.iter().rev().map(|code| Injected::Key {
            code: *code,
            pressed: false,
        }));
        self.inject(&injected);
    }

    pub(crate) fn x11(&self) -> (RustConnection, usize) {
        let display = self
            .call(|compositor| compositor.x_display())
            .expect("Xwayland started");
        x11rb::connect(Some(&format!(":{display}"))).expect("Xwayland accepts a connection")
    }

    pub(crate) fn frame_where(
        &self,
        predicate: impl Fn(&Snapshot) -> bool + Send + 'static,
    ) -> Snapshot {
        let (matched, receiver) = mpsc::channel();
        self.call(move |compositor| {
            let reactor = compositor.reactor().clone();
            let hook = Rc::new(Cell::new(None));
            let installed = Rc::clone(&hook);
            let token = reactor
                .clone()
                .each_turn(move |compositor: &mut Compositor| {
                    let Some(snapshot) =
                        compositor.snapshot().filter(|snapshot| predicate(snapshot))
                    else {
                        return;
                    };
                    drop(matched.send(snapshot));
                    if let Some(token) = installed.take() {
                        reactor.remove(token);
                    }
                });
            hook.set(Some(token));
        });
        match receiver.recv_timeout(PATIENCE) {
            Ok(snapshot) => snapshot,
            Err(_) => panic!(
                "timed out waiting for frame; last frame: {:?}",
                self.call(Compositor::snapshot)
            ),
        }
    }
}

pub(crate) fn shows(snapshot: &Snapshot, x: u32, y: u32, expected: [u8; 4]) -> bool {
    snapshot.pixel(x, y).is_some_and(|pixel| {
        pixel
            .iter()
            .zip(expected)
            .all(|(channel, expected)| channel.abs_diff(expected) <= 2)
    })
}

pub(crate) fn mapped(client: &mut Client, tag: u32, size: (i32, i32), bgra: [u8; 4]) -> Toplevel {
    let window = client.toplevel(tag);
    client.ack(&window.xdg, tag);
    let pixels = client.pixels(size.0, size.1, tag);
    pixels.fill(0..size.1, bgra);
    client.show(&window.surface, &pixels);
    window
}

pub(crate) fn tile(client: &mut Client, tag: u32, bgra: [u8; 4]) -> Toplevel {
    let window = client.toplevel(tag);
    let configure = client
        .received
        .shell
        .last_configure(tag)
        .cloned()
        .expect("tiled window configured on first commit");
    client.ack(&window.xdg, tag);
    let pixels = client.pixels(configure.width, configure.height, tag);
    pixels.fill(0..configure.height, bgra);
    client.show(&window.surface, &pixels);
    window
}

impl Drop for Harness {
    fn drop(&mut self) {
        if let Some(tasks) = self.tasks.take() {
            let _ = tasks.send(Box::new(Compositor::stop));
        }
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
            && !std::thread::panicking()
        {
            panic!("the compositor thread panicked");
        }
    }
}

impl Drop for FakeShell {
    fn drop(&mut self) {
        match self.path.take() {
            Some(path) => unsafe { std::env::set_var("PATH", path) },
            None => unsafe { std::env::remove_var("PATH") },
        }
    }
}

impl FakeShell {
    fn install(script: &str) -> FakeShell {
        let bin = tempfile::tempdir().unwrap();
        let program = bin.path().join("rikka-shell");
        std::fs::write(&program, script).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = std::env::var_os("PATH");
        let searched = std::env::join_paths(
            std::iter::once(bin.path().to_path_buf())
                .chain(path.iter().flat_map(std::env::split_paths)),
        )
        .unwrap();
        unsafe { std::env::set_var("PATH", searched) };
        FakeShell { _bin: bin, path }
    }
}

pub(crate) fn interfaces<R>(client: &rikka_test_display::Client<R>) -> Vec<String> {
    client
        .globals()
        .into_iter()
        .map(|global| global.interface.to_string())
        .collect()
}
