use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::time::Instant;
use std::{fmt, io};

use rustix::event::epoll::{self, CreateFlags, Event, EventData, EventFlags};
use rustix::event::{EventfdFlags, eventfd};
use rustix::time::{
    Itimerspec, TimerfdClockId, TimerfdFlags, TimerfdTimerFlags, Timespec, timerfd_create,
    timerfd_settime,
};

const CONNECTION: u64 = 0;
const WAKE: u64 = 1;
const TIMER: u64 = 2;
const FIRST_SOURCE: u64 = 3;
const CAPACITY: usize = 32;
const NOW: Timespec = Timespec {
    tv_sec: 0,
    tv_nsec: 0,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct SourceId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Direction {
    Read,
    Write,
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Ready {
    pub(crate) connection: bool,
    pub(crate) timer: bool,
}

pub(crate) struct Poller {
    epoll: OwnedFd,
    wake: OwnedFd,
    timer: OwnedFd,
    armed: Option<Instant>,
    issued: u64,
    woken: bool,
    writing: bool,
    events: Vec<Event>,
    sources: Vec<SourceId>,
}

impl Poller {
    pub(crate) fn new(connection: BorrowedFd<'_>) -> io::Result<Self> {
        let epoll = epoll::create(CreateFlags::CLOEXEC)?;
        let wake = eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK)?;
        let timer = timerfd_create(
            TimerfdClockId::Monotonic,
            TimerfdFlags::CLOEXEC | TimerfdFlags::NONBLOCK,
        )?;
        epoll::add(
            &epoll,
            connection,
            EventData::new_u64(CONNECTION),
            EventFlags::IN,
        )?;
        epoll::add(&epoll, &wake, EventData::new_u64(WAKE), EventFlags::IN)?;
        epoll::add(&epoll, &timer, EventData::new_u64(TIMER), EventFlags::IN)?;
        Ok(Self {
            epoll,
            wake,
            timer,
            armed: None,
            issued: FIRST_SOURCE,
            woken: false,
            writing: false,
            events: Vec::with_capacity(CAPACITY),
            sources: Vec::with_capacity(CAPACITY),
        })
    }

    pub(crate) fn source(&mut self) -> SourceId {
        let id = SourceId(self.issued);
        self.issued += 1;
        id
    }

    pub(crate) fn watch(
        &self,
        fd: BorrowedFd<'_>,
        source: SourceId,
        direction: Direction,
    ) -> io::Result<()> {
        let flags = match direction {
            Direction::Read => EventFlags::IN,
            Direction::Write => EventFlags::OUT,
        };
        epoll::add(
            &self.epoll,
            fd,
            EventData::new_u64(source.0),
            flags | EventFlags::ET,
        )?;
        Ok(())
    }

    pub(crate) fn unwatch(&self, fd: BorrowedFd<'_>) {
        if let Err(error) = epoll::delete(&self.epoll, fd) {
            tracing::debug!("failed to unregister selection transfer: {error}");
        }
    }

    pub(crate) fn wake(&mut self) {
        if self.woken {
            return;
        }
        match rustix::io::write(&self.wake, &1u64.to_ne_bytes()) {
            Ok(_) => self.woken = true,
            Err(error) => tracing::debug!("failed to wake X11 window manager: {error}"),
        }
    }

    pub(crate) fn want_writable(
        &mut self,
        connection: BorrowedFd<'_>,
        writable: bool,
    ) -> io::Result<()> {
        if writable == self.writing {
            return Ok(());
        }
        let flags = if writable {
            EventFlags::IN | EventFlags::OUT
        } else {
            EventFlags::IN
        };
        epoll::modify(
            &self.epoll,
            connection,
            EventData::new_u64(CONNECTION),
            flags,
        )?;
        self.writing = writable;
        Ok(())
    }

    pub(crate) fn arm(&mut self, deadline: Option<Instant>) -> io::Result<()> {
        let Some(deadline) = deadline else {
            return Ok(());
        };
        if self.armed.is_some_and(|armed| armed <= deadline) {
            return Ok(());
        }
        let delay = deadline.saturating_duration_since(Instant::now());
        timerfd_settime(
            &self.timer,
            TimerfdTimerFlags::empty(),
            &Itimerspec {
                it_interval: NOW,
                it_value: Timespec {
                    tv_sec: i64::try_from(delay.as_secs()).unwrap_or(i64::MAX),
                    tv_nsec: i64::from(delay.subsec_nanos().max(1)),
                },
            },
        )?;
        self.armed = Some(deadline);
        Ok(())
    }

    pub(crate) fn ready(&mut self) -> io::Result<Ready> {
        self.events.clear();
        self.sources.clear();
        match epoll::wait(
            &self.epoll,
            rustix::buffer::spare_capacity(&mut self.events),
            Some(&NOW),
        ) {
            Ok(_) | Err(rustix::io::Errno::INTR) => {}
            Err(error) => return Err(error.into()),
        }
        let mut ready = Ready::default();
        for event in &self.events {
            match event.data.u64() {
                CONNECTION => ready.connection = true,
                WAKE => {
                    drain(&self.wake);
                    self.woken = false;
                }
                TIMER => {
                    drain(&self.timer);
                    self.armed = None;
                    ready.timer = true;
                }
                source => self.sources.push(SourceId(source)),
            }
        }
        Ok(ready)
    }

    pub(crate) fn take_sources(&mut self) -> Vec<SourceId> {
        std::mem::take(&mut self.sources)
    }

    pub(crate) fn return_sources(&mut self, sources: Vec<SourceId>) {
        self.sources = sources;
    }
}

impl AsFd for Poller {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.epoll.as_fd()
    }
}

fn drain(fd: &OwnedFd) {
    let mut counter = [0u8; 8];
    if let Err(error) = rustix::io::read(fd, &mut counter)
        && error != rustix::io::Errno::AGAIN
    {
        tracing::debug!("failed to drain X11 window manager wakeup: {error}");
    }
}

impl fmt::Debug for Poller {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Poller")
            .field("armed", &self.armed)
            .field("woken", &self.woken)
            .field("writing", &self.writing)
            .finish_non_exhaustive()
    }
}
