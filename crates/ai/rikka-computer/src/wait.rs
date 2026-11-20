use std::time::{Duration, Instant};

use rikka_capture::{Capturer, Screen, ScreenWindow, Source};

use crate::error::{Error, Result, capture};
use crate::shot::CURSOR;

pub(crate) const QUIET_PERIOD: Duration = Duration::from_millis(300);

pub(crate) fn wait_for_window(
    mut screen: Screen,
    matches: &dyn Fn(&ScreenWindow) -> bool,
    timeout: Duration,
) -> Result<ScreenWindow> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(window) = screen.windows().into_iter().find(|window| matches(window)) {
            return Ok(window);
        }
        if !screen.dispatch(Some(deadline)).map_err(capture)? {
            return Err(Error::WindowTimeout(timeout));
        }
    }
}

pub(crate) fn wait_idle(screen: Screen, source: &Source, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut capturer = Capturer::open(screen, source, CURSOR).map_err(capture)?;
    if capturer
        .take_before(Some(deadline))
        .map_err(capture)?
        .is_none()
    {
        return Err(Error::IdleTimeout(timeout));
    }
    loop {
        let quiet_until = Instant::now() + QUIET_PERIOD;
        if quiet_until > deadline {
            return Err(Error::IdleTimeout(timeout));
        }
        if capturer
            .take_before(Some(quiet_until))
            .map_err(capture)?
            .is_none()
        {
            return Ok(());
        }
    }
}
