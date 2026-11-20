use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::JoinHandle;

use rikka_compositor_reactor::{Reactor, Sender};
use rikka_compositor_seat::{Keymap, SeatHost, Seats};
use rikka_compositor_selection::{SelectionHost, SelectionProtocols, Selections};
use rikka_compositor_server::{DisplayHandle, Socket};
use rikka_compositor_shell::{Shell, ShellEvent, ShellHost, TokenRequest, XdgShell};
use rikka_compositor_surface::{SurfaceHost, SurfaceProtocols, Surfaces};

use super::KEYMAP;

struct State {
    display: DisplayHandle<State>,
    surfaces: Surfaces,
    seats: Seats,
    shell: Shell,
    selections: Selections,
    stopped: bool,
}

impl SurfaceHost for State {
    fn surfaces(&mut self) -> &mut Surfaces {
        &mut self.surfaces
    }
}

impl SeatHost for State {
    fn seats(&mut self) -> &mut Seats {
        &mut self.seats
    }

    fn display(&self) -> &DisplayHandle<Self> {
        &self.display
    }
}

impl ShellHost for State {
    fn shell(&mut self) -> &mut Shell {
        &mut self.shell
    }

    fn shell_event(&mut self, _event: ShellEvent) {}

    fn admit_activation(&mut self, _request: &TokenRequest) -> bool {
        false
    }
}

impl SelectionHost for State {
    fn selections(&mut self) -> &mut Selections {
        &mut self.selections
    }
}

pub(crate) struct Session {
    stop: Sender<()>,
    thread: Option<JoinHandle<()>>,
    path: PathBuf,
}

impl Session {
    pub(crate) fn start() -> Session {
        let runtime = std::env::var_os("XDG_RUNTIME_DIR").expect("a runtime directory");
        let path = Path::new(&runtime).join(format!("session-{}", std::process::id()));
        let (ready, answer) = mpsc::channel();
        let socket = path.clone();
        let thread = std::thread::Builder::new()
            .name("rikka-test-session".into())
            .spawn(move || {
                let reactor = Reactor::<State>::new().unwrap();
                let display = DisplayHandle::new(&reactor);
                display.install(SurfaceProtocols);
                display.install(rikka_compositor_seat::SeatProtocols);
                display.install(XdgShell::new());
                display.install(SelectionProtocols);
                let shell = Shell::new(&display);
                let mut seats = Seats::new();
                seats.add(&display, "seat0", Keymap::from_text(KEYMAP).unwrap());
                let listening: Socket = display.listen(socket.to_str()).unwrap();
                let mut state = State {
                    display,
                    surfaces: Surfaces::new(),
                    seats,
                    shell,
                    selections: Selections::new(),
                    stopped: false,
                };
                let stop = reactor
                    .mailbox(|state: &mut State, ()| state.stopped = true)
                    .unwrap();
                ready.send(stop).unwrap();
                reactor.run(&mut state, |state| state.stopped).unwrap();
                drop(listening);
            })
            .unwrap();
        Session {
            stop: answer.recv().unwrap(),
            thread: Some(thread),
            path,
        }
    }

    pub(crate) fn clipboard(&self) -> rikka_clipboard::Clipboard {
        let display = rikka_wayland::Display::connect_to(&self.path).unwrap();
        let host = rikka_wayland::Host::with_reader(display).unwrap();
        rikka_clipboard::Clipboard::for_host(&host).unwrap()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
