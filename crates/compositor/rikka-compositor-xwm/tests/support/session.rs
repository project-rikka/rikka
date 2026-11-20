use std::os::fd::{AsFd, BorrowedFd};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rikka_compositor_xwm::{Xwayland, Xwm, XwmEvent};
use rustix::event::{PollFd, PollFlags, poll};
use rustix::time::Timespec;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

use crate::support::wayland::{Serials, serve};
use crate::support::x11::drain;

const PATIENCE: Duration = Duration::from_secs(20);
const FALLBACK_DIRECTORIES: [&str; 3] =
    ["/run/current-system/sw/bin", "/usr/bin", "/usr/local/bin"];

pub(crate) struct Session {
    pub(crate) xwm: Xwm,
    pub(crate) serials: Arc<Serials>,
    display: String,
    received: Vec<XwmEvent>,
    _xwayland: Xwayland,
}

impl Session {
    pub(crate) fn start() -> Self {
        let (mut xwayland, wayland) = Xwayland::launch()
            .program(xwayland_program())
            .spawn()
            .expect("Xwayland starts");
        let serials = serve(wayland);
        let deadline = Instant::now() + PATIENCE;
        let connection = loop {
            assert!(
                wait_readable(&[xwayland.ready_fd()], deadline),
                "Xwayland never became ready"
            );
            if let Some(connection) = xwayland.ready().expect("Xwayland stays up") {
                break connection;
            }
        };
        let display = xwayland.display();
        let xwm = Xwm::start(connection).expect("the window manager starts");
        Self {
            xwm,
            serials,
            display,
            received: Vec::new(),
            _xwayland: xwayland,
        }
    }

    pub(crate) fn client(&self) -> RustConnection {
        RustConnection::connect(Some(&self.display))
            .expect("an X11 client connects")
            .0
    }

    pub(crate) fn wait_for(
        &mut self,
        what: &str,
        mut pick: impl FnMut(&XwmEvent) -> bool,
    ) -> XwmEvent {
        self.wait_for_serving(what, &[], &mut || {}, &mut pick)
    }

    pub(crate) fn wait_for_serving(
        &mut self,
        what: &str,
        peers: &[BorrowedFd<'_>],
        service: &mut dyn FnMut(),
        mut pick: impl FnMut(&XwmEvent) -> bool,
    ) -> XwmEvent {
        let deadline = Instant::now() + PATIENCE;
        loop {
            service();
            if let Some(index) = self.received.iter().position(&mut pick) {
                return self.received.remove(index);
            }
            assert!(
                self.turn(peers, deadline),
                "gave up waiting for {what}; saw {:#?}",
                self.received
            );
        }
    }

    pub(crate) fn run_until(
        &mut self,
        what: &str,
        peers: &[BorrowedFd<'_>],
        mut step: impl FnMut(&mut Xwm) -> bool,
    ) {
        let deadline = Instant::now() + PATIENCE;
        while !step(&mut self.xwm) {
            assert!(self.turn(peers, deadline), "gave up waiting for {what}");
        }
    }

    pub(crate) fn wait_for_serial(&self, serial: u64) {
        self.serials.wait_for(serial, Instant::now() + PATIENCE);
    }

    pub(crate) fn flush(&mut self) {
        self.xwm
            .flush()
            .expect("the window manager keeps its connection");
    }

    fn turn(&mut self, peers: &[BorrowedFd<'_>], deadline: Instant) -> bool {
        self.flush();
        let mut fds = vec![self.xwm.as_fd()];
        fds.extend_from_slice(peers);
        let ready = wait_readable(&fds, deadline);
        self.xwm
            .dispatch_into(&mut self.received)
            .expect("the window manager keeps its connection");
        self.flush();
        ready
    }
}

pub(crate) fn wait_until(
    app: &RustConnection,
    what: &str,
    mut holds: impl FnMut(&[Event]) -> bool,
) {
    let deadline = Instant::now() + PATIENCE;
    let mut events = drain(app);
    while !holds(&events) {
        events = drain(app);
        if events.is_empty() {
            assert!(
                wait_readable(&[app.stream().as_fd()], deadline),
                "gave up waiting for {what}"
            );
            events = drain(app);
        }
    }
}

fn wait_readable(fds: &[BorrowedFd<'_>], deadline: Instant) -> bool {
    let mut polled: Vec<PollFd<'_>> = fds
        .iter()
        .map(|fd| PollFd::new(fd, PollFlags::IN))
        .collect();
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let timeout = Timespec {
            tv_sec: left.as_secs() as i64,
            tv_nsec: i64::from(left.subsec_nanos()),
        };
        match poll(&mut polled, Some(&timeout)) {
            Ok(ready) => return ready > 0,
            Err(rustix::io::Errno::INTR) => {}
            Err(error) => panic!("polling failed: {error}"),
        }
    }
}

pub(crate) fn xwayland_program() -> PathBuf {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .chain(FALLBACK_DIRECTORIES.iter().map(PathBuf::from))
        .map(|directory| directory.join("Xwayland"))
        .find(|candidate| candidate.is_file())
        .expect("this machine has Xwayland")
}
