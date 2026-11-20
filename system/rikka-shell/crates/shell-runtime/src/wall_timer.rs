use std::os::fd::{AsFd, BorrowedFd};
use std::time::SystemTime;

use rikka_worker::WallAlarm;

use shell_kit::WallClock;

pub(super) struct WallTimer {
    wall: WallClock,
    alarm: WallAlarm,
    armed: Option<SystemTime>,
}

impl WallTimer {
    pub(super) fn new(wall: WallClock) -> std::io::Result<WallTimer> {
        Ok(WallTimer {
            wall,
            alarm: WallAlarm::new()?,
            armed: None,
        })
    }

    pub(super) fn follow(&mut self) {
        if self.wall.take_moved() {
            self.arm();
        }
    }

    pub(super) fn ring(&mut self) {
        match self.alarm.read() {
            Ok(Some(_)) => {
                self.wall.advance();
                self.armed = None;
                self.arm();
            }
            Ok(None) => {}
            Err(error) => tracing::warn!("failed to read wall clock timer: {error}"),
        }
    }

    fn arm(&mut self) {
        let next = self.wall.next();
        if next == self.armed {
            return;
        }
        let armed = match next {
            Some(at) => self.alarm.arm(at),
            None => self.alarm.disarm(),
        };
        match armed {
            Ok(()) => self.armed = next,
            Err(error) => tracing::warn!("failed to arm wall clock timer: {error}"),
        }
    }
}

impl AsFd for WallTimer {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.alarm.as_fd()
    }
}
