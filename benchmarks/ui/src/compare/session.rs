use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use futures_util::StreamExt;
use futures_util::future::{self, Either};
use rikka_seam::net::SocketClient;
use rikka_session::agent;
use rikka_session::toplevels::{self, Bounds, State, ToplevelsClient};
use rikka_types::{Key, PointerButton};
use rikka_virtual_input::{AbsolutePosition, Cap, Keyboard, Pointer, Seat};
use tempfile::TempDir;
use ui_scene::monotonic_ns;

use super::error::Error;
use super::framework::{Display, InputSeat};
use super::process::children_named;
use super::supervised::Supervised;

pub(crate) const OUTPUT_WIDTH: u32 = 1920;

pub(crate) const OUTPUT_HEIGHT: u32 = 1080;

const START_TIMEOUT: Duration = Duration::from_secs(30);

const WINDOW_TIMEOUT: Duration = Duration::from_secs(30);

const XWAYLAND_READY: &str = "Xwayland ready on ";

const SESSION_VARIABLES: [&str; 6] = [
    "WAYLAND_DISPLAY",
    "WAYLAND_SOCKET",
    "DISPLAY",
    "RIKKA_SEAM_SOCKET",
    "DBUS_SESSION_BUS_ADDRESS",
    "XDG_RUNTIME_DIR",
];

pub(crate) struct Session {
    person: SeatInput,
    agent: SeatInput,
    seam: SocketClient,
    compositor: Supervised,
    root: TempDir,
    socket: String,
    x_display: String,
}

struct SeatInput {
    _seat: Seat,
    pointer: Pointer,
    _keyboard: Keyboard,
}

impl SeatInput {
    fn person() -> Result<SeatInput, Error> {
        SeatInput::new(Seat::connect().map_err(input_error)?)
    }

    fn agent() -> Result<SeatInput, Error> {
        let socket = agent::socket_path()
            .ok_or_else(|| Error::Input("no runtime directory for the agent socket".to_owned()))?;
        SeatInput::new(Seat::connect_named(&socket, agent::SEAT).map_err(input_error)?)
    }

    fn new(seat: Seat) -> Result<SeatInput, Error> {
        let pointer = seat.pointer().map_err(input_error)?;
        let keyboard = seat
            .keyboard([Cap::Named(Key::Escape)])
            .map_err(input_error)?;
        Ok(SeatInput {
            _seat: seat,
            pointer,
            _keyboard: keyboard,
        })
    }
}

fn input_error(error: impl std::fmt::Display) -> Error {
    Error::Input(error.to_string())
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Shown {
    pub(crate) bounds: Bounds,
    pub(crate) at: u64,
}

impl Session {
    pub(crate) fn start(compositor: &Path) -> Result<Session, Error> {
        let root = tempfile::Builder::new()
            .prefix("ui-bench-")
            .tempdir()
            .map_err(|error| {
                Error::Compositor(format!("failed to create a scratch directory: {error}"))
            })?;
        let runtime = root.path().join("runtime");
        create_private_directory(&runtime)?;
        let socket = format!("ui-bench-{}", std::process::id());
        unsafe {
            for variable in SESSION_VARIABLES {
                std::env::remove_var(variable);
            }
            std::env::set_var("XDG_RUNTIME_DIR", &runtime);
            std::env::set_var("WAYLAND_DISPLAY", &socket);
        }
        let mut command = Command::new(compositor);
        command
            .arg(format!("--headless={OUTPUT_WIDTH}x{OUTPUT_HEIGHT}"))
            .arg("--no-shell")
            .arg("--socket")
            .arg(&socket)
            .env("RUST_LOG", "info")
            .envs(scratch_variables(root.path()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let mut compositor_child =
            Supervised::spawn(&mut command).map_err(|source| Error::Spawn {
                program: compositor.to_owned(),
                source,
            })?;
        let stderr = compositor_child
            .take_stderr()
            .expect("compositor stderr is piped");
        let log_path = root.path().join("compositor.log");
        let mut log = std::fs::File::create(&log_path)
            .map_err(|error| Error::Compositor(format!("failed to create its log: {error}")))?;
        let (ready, announced) = mpsc::channel();
        std::thread::Builder::new()
            .name("rikka-ui-bench-compositor-log".to_owned())
            .spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    if let Some(display) = x_display_in(&line) {
                        let _ = ready.send(display);
                    }
                    if writeln!(log, "{line}").is_err() {
                        break;
                    }
                }
            })
            .map_err(|error| Error::Compositor(format!("failed to read its log: {error}")))?;
        let x_display = announced.recv_timeout(START_TIMEOUT).map_err(|_| {
            Error::Compositor(format!(
                "Xwayland did not become ready, see {}",
                log_path.display()
            ))
        })?;
        let seam = SocketClient::from_env()
            .map_err(|error| Error::Compositor(format!("no session socket: {error}")))?;
        Ok(Session {
            person: SeatInput::person()?,
            agent: SeatInput::agent()?,
            seam,
            compositor: compositor_child,
            root,
            socket,
            x_display,
        })
    }

    #[must_use]
    pub(crate) fn root(&self) -> &Path {
        self.root.path()
    }

    #[must_use]
    pub(crate) fn command(&self, program: &Path, display: Display) -> Command {
        let mut command = Command::new(program);
        command.envs(scratch_variables(&self.root.path().join("client")));
        match display {
            Display::Wayland => command.env("WAYLAND_DISPLAY", &self.socket),
            Display::X11 => command
                .env_remove("WAYLAND_DISPLAY")
                .env("DISPLAY", &self.x_display),
        };
        command
    }

    #[must_use]
    pub(crate) fn server_pids(&self) -> Vec<u32> {
        let compositor = self.compositor.id();
        let mut pids = vec![compositor];
        pids.extend(children_named(compositor, "Xwayland"));
        pids
    }

    pub(crate) fn wait_for_window(&self, seat: InputSeat) -> Result<Shown, Error> {
        let shown = self.wait_for("the window to show", |state| {
            state
                .toplevels
                .iter()
                .find(|toplevel| toplevel.on_screen && toplevel.bounds.width > 0)
                .map(|toplevel| Shown {
                    bounds: toplevel.bounds,
                    at: monotonic_ns(),
                })
        })?;
        self.click(
            seat,
            shown.bounds.x + shown.bounds.width / 2,
            shown.bounds.y + shown.bounds.height / 2,
        )?;
        Ok(shown)
    }

    pub(crate) fn wait_until_empty(&self) -> Result<(), Error> {
        self.wait_for("the previous window to close", |state| {
            state.toplevels.is_empty().then_some(())
        })
    }

    pub(crate) fn point(&self, seat: InputSeat, x: i32, y: i32) -> Result<(), Error> {
        let clamp = |value: i32, extent: u32| value.clamp(0, extent as i32 - 1) as u32;
        self.input(seat)
            .pointer
            .move_to(AbsolutePosition::new(
                clamp(x, OUTPUT_WIDTH),
                clamp(y, OUTPUT_HEIGHT),
                OUTPUT_WIDTH,
                OUTPUT_HEIGHT,
            ))
            .map_err(|error| Error::Input(error.to_string()))
    }

    fn click(&self, seat: InputSeat, x: i32, y: i32) -> Result<(), Error> {
        self.point(seat, x, y)?;
        let pointer = &self.input(seat).pointer;
        pointer
            .press(PointerButton::Primary)
            .and_then(|()| pointer.release(PointerButton::Primary))
            .map_err(|error| Error::Input(error.to_string()))
    }

    fn input(&self, seat: InputSeat) -> &SeatInput {
        match seat {
            InputSeat::Person => &self.person,
            InputSeat::Agent => &self.agent,
        }
    }

    fn wait_for<T>(
        &self,
        awaited: &str,
        mut accept: impl FnMut(&State) -> Option<T>,
    ) -> Result<T, Error> {
        let client = self.seam.client();
        let changes = Box::pin(toplevels::states(&client));
        let current: State = async_io::block_on(ToplevelsClient::new(client).state())
            .and_then(|payload| payload.decode())
            .map_err(|error| Error::Window(format!("failed to read the toplevels: {error}")))?;
        if let Some(value) = accept(&current) {
            return Ok(value);
        }
        let mut accepted = changes.filter_map(|state| future::ready(accept(&state)));
        let timeout = async_io::Timer::after(WINDOW_TIMEOUT);
        match async_io::block_on(future::select(accepted.next(), timeout)) {
            Either::Left((Some(value), _)) => Ok(value),
            Either::Left((None, _)) => Err(Error::Window("the toplevel stream ended".to_owned())),
            Either::Right(_) => Err(Error::Window(format!("timed out waiting for {awaited}"))),
        }
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Session")
            .field("socket", &self.socket)
            .field("x_display", &self.x_display)
            .finish_non_exhaustive()
    }
}

fn scratch_variables(root: &Path) -> Vec<(&'static str, OsString)> {
    [
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_STATE_HOME", "state"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_CACHE_HOME", "cache"),
    ]
    .into_iter()
    .map(|(name, folder)| (name, root.join(folder).into_os_string()))
    .collect()
}

fn create_private_directory(path: &Path) -> Result<(), Error> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(path)
        .map_err(|error| Error::Compositor(format!("failed to create {}: {error}", path.display())))
}

fn x_display_in(line: &str) -> Option<String> {
    let after = &line[line.find(XWAYLAND_READY)? + XWAYLAND_READY.len()..];
    let digits: String = after
        .strip_prefix(':')?
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    (!digits.is_empty()).then(|| format!(":{digits}"))
}
