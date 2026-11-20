mod bridge;
mod convert;
mod door;
mod embedded_views;
mod error;
mod handlers;
mod idling;
mod layer;
mod locking;
mod opacity;
mod role;
mod screens;
mod senders;
mod state;
mod surface_state;
mod surfaces;
mod timers;
mod wall_timer;

use std::os::fd::AsFd as _;
use std::sync::Arc;
use std::task::Waker;
use std::time::Instant;

pub use error::{Error, Result};
use rikka_wayland::Display;
use shell_kit::Shell;
use state::ShellState;
use wall_timer::WallTimer;

pub fn run(shell: Shell) -> Result<()> {
    let parts = shell.into_parts()?;
    if let Some(session) = parts.host.clipboard() {
        rikka_ui::clipboard::install_session(session);
    }
    let display = Display::from_env()?;
    let mut event_loop = display.event_loop::<ShellState>();
    let wake_fd = parts.waker.install()?;
    event_loop
        .queue_mut()
        .set_waker(Waker::from(Arc::new(parts.waker.clone())));
    let mut wall = WallTimer::new(parts.wall.clone()).map_err(Error::WallClock)?;
    handlers::listen(&mut event_loop);
    let mut state = ShellState::new(parts, display, event_loop.route())?;

    event_loop.roundtrip(&mut state)?;
    event_loop.roundtrip(&mut state)?;
    state.reconcile();

    while !state.stopped() {
        wall.follow();
        let wait = timers::wait_until(state.deadline(), Instant::now());
        let mut clock_fired = false;
        let dispatched = event_loop.blocking_dispatch(&mut state, |fd| {
            let [display, clock] = wake_fd.poll_readable([fd, wall.as_fd()], wait)?;
            clock_fired = clock;
            Ok(display)
        });
        if let Err(error) = dispatched {
            state.fail(error.into());
            break;
        }
        wake_fd.drain();
        if clock_fired {
            wall.ring();
        }
        state.read_mail();
        state.ring(Instant::now());
        state.about_to_wait();
    }
    state.finish()
}
