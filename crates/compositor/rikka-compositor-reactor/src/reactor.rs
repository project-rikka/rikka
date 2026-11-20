use std::cell::{Cell, RefCell};
use std::fmt;
use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
use std::rc::Rc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use rustix::event::epoll::{self, CreateFlags, Event, EventData, EventFlags};
use rustix::time::{
    Itimerspec, TimerfdClockId, TimerfdFlags, TimerfdTimerFlags, Timespec, timerfd_create,
    timerfd_settime,
};

use crate::interest::{Interest, Ready};
use crate::mailbox::{Notifier, Sender};
use crate::sources::{Source, Sources, TakenCallback};
use crate::timers::Timers;
use crate::weak::WeakReactor;

const TIMER_TOKEN: u64 = 0;
const EVENT_CAPACITY: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Token(u64);

type IdleFn<S> = Box<dyn FnOnce(&mut S)>;
type SettleFn<S> = Box<dyn FnMut(&mut S)>;

struct Inner<S> {
    sources: Sources<S>,
    timers: Timers,
    armed: Option<Instant>,
    idle: Vec<IdleFn<S>>,
    woken: Vec<u64>,
}

impl<S> Inner<S> {
    fn busy(&self) -> bool {
        !self.idle.is_empty() || !self.woken.is_empty()
    }
}

pub(crate) struct Shared<S> {
    epoll: OwnedFd,
    timerfd: OwnedFd,
    inner: RefCell<Inner<S>>,
    events: Cell<Vec<Event>>,
    hooks: Cell<Vec<u64>>,
    idle: Cell<Vec<IdleFn<S>>>,
    settle: Cell<Option<SettleFn<S>>>,
}

pub struct Reactor<S: 'static> {
    shared: Rc<Shared<S>>,
}

impl<S: 'static> Clone for Reactor<S> {
    fn clone(&self) -> Self {
        Self {
            shared: self.shared.clone(),
        }
    }
}

impl<S: 'static> fmt::Debug for Reactor<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Reactor")
            .field("epoll", &self.shared.epoll.as_raw_fd())
            .finish_non_exhaustive()
    }
}

impl<S: 'static> AsFd for Reactor<S> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.shared.epoll.as_fd()
    }
}

impl<S: 'static> Reactor<S> {
    pub(crate) fn from_shared(shared: Rc<Shared<S>>) -> Self {
        Self { shared }
    }

    pub fn downgrade(&self) -> WeakReactor<S> {
        WeakReactor::new(Rc::downgrade(&self.shared))
    }

    pub fn new() -> io::Result<Self> {
        let epoll = epoll::create(CreateFlags::CLOEXEC)?;
        let timerfd = timerfd_create(
            TimerfdClockId::Monotonic,
            TimerfdFlags::CLOEXEC | TimerfdFlags::NONBLOCK,
        )?;
        epoll::add(
            &epoll,
            &timerfd,
            EventData::new_u64(TIMER_TOKEN),
            EventFlags::IN,
        )?;
        Ok(Self {
            shared: Rc::new(Shared {
                epoll,
                timerfd,
                inner: RefCell::new(Inner {
                    sources: Sources::new(),
                    timers: Timers::new(),
                    armed: None,
                    idle: Vec::new(),
                    woken: Vec::new(),
                }),
                events: Cell::new(Vec::with_capacity(EVENT_CAPACITY)),
                hooks: Cell::new(Vec::new()),
                idle: Cell::new(Vec::new()),
                settle: Cell::new(None),
            }),
        })
    }

    pub fn watch(
        &self,
        fd: BorrowedFd<'_>,
        interest: Interest,
        on: impl FnMut(&mut S, Ready) + 'static,
    ) -> io::Result<Token> {
        let token = self.insert(Source::Watch {
            fd: fd.as_raw_fd(),
            callback: Some(Box::new(on)),
        });
        if let Err(error) = epoll::add(
            &self.shared.epoll,
            fd,
            EventData::new_u64(token),
            interest.flags(),
        ) {
            self.shared.inner.borrow_mut().sources.remove(token);
            return Err(error.into());
        }
        Ok(Token(token))
    }

    pub fn set_interest(&self, token: Token, interest: Interest) -> io::Result<()> {
        let Some(fd) = self.shared.inner.borrow().sources.fd_of(token.0) else {
            return Ok(());
        };
        let fd = unsafe { BorrowedFd::borrow_raw(fd) };
        epoll::modify(
            &self.shared.epoll,
            fd,
            EventData::new_u64(token.0),
            interest.flags(),
        )?;
        Ok(())
    }

    pub fn timer(&self, at: Instant, on: impl FnOnce(&mut S) + 'static) -> Token {
        let mut inner = self.shared.inner.borrow_mut();
        let token = inner.sources.insert(Source::Timer {
            at,
            callback: Box::new(on),
        });
        inner.timers.schedule(token, at);
        self.arm(&mut inner);
        Token(token)
    }

    pub fn after(&self, delay: Duration, on: impl FnOnce(&mut S) + 'static) -> Token {
        self.timer(Instant::now() + delay, on)
    }

    pub fn mailbox<M: Send + 'static>(
        &self,
        mut on: impl FnMut(&mut S, M) + 'static,
    ) -> io::Result<Sender<M>> {
        let notifier = Notifier::new()?;
        let (sender, receiver) = mpsc::channel::<M>();
        let drain = move |state: &mut S| {
            while let Ok(message) = receiver.try_recv() {
                on(state, message);
            }
        };
        let token = self.insert(Source::Mailbox {
            notifier: notifier.clone(),
            drain: Some(Box::new(drain)),
        });
        if let Err(error) = epoll::add(
            &self.shared.epoll,
            notifier.fd(),
            EventData::new_u64(token),
            EventFlags::IN,
        ) {
            self.shared.inner.borrow_mut().sources.remove(token);
            return Err(error.into());
        }
        Ok(Sender::new(sender, notifier))
    }

    pub fn idle(&self, on: impl FnOnce(&mut S) + 'static) {
        self.shared.inner.borrow_mut().idle.push(Box::new(on));
    }

    pub fn task(&self, on: impl FnMut(&mut S) + 'static) -> Token {
        Token(self.insert(Source::Task {
            callback: Some(Box::new(on)),
            woken: false,
        }))
    }

    pub fn wake(&self, token: Token) {
        let mut inner = self.shared.inner.borrow_mut();
        if inner.sources.wake(token.0) {
            inner.woken.push(token.0);
        }
    }

    pub fn each_turn(&self, on: impl FnMut(&mut S) + 'static) -> Token {
        Token(self.insert(Source::EachTurn {
            callback: Some(Box::new(on)),
        }))
    }

    pub fn after_each(&self, settle: impl FnMut(&mut S) + 'static) {
        self.shared.settle.set(Some(Box::new(settle)));
    }

    pub fn remove(&self, token: Token) {
        let mut inner = self.shared.inner.borrow_mut();
        match inner.sources.remove(token.0) {
            Some(Source::Timer { .. }) => {
                inner.timers.cancelled();
                self.arm(&mut inner);
            }
            Some(Source::Watch { fd, .. }) => {
                if !inner.sources.watches_fd(fd) {
                    let fd = unsafe { BorrowedFd::borrow_raw(fd) };
                    let _ = epoll::delete(&self.shared.epoll, fd);
                }
            }
            Some(Source::Mailbox { notifier, .. }) => {
                notifier.close();
                let _ = epoll::delete(&self.shared.epoll, notifier.fd());
            }
            Some(Source::EachTurn { .. } | Source::Task { .. }) | None => {}
        }
    }

    pub fn run(&self, state: &mut S, mut until: impl FnMut(&S) -> bool) -> io::Result<()> {
        while !until(state) {
            self.turn(state, None)?;
        }
        Ok(())
    }

    pub fn turn(&self, state: &mut S, timeout: Option<Duration>) -> io::Result<()> {
        let timeout = if self.shared.inner.borrow().busy() {
            Some(Duration::ZERO)
        } else {
            timeout
        };
        let mut events = self.shared.events.take();
        events.clear();
        events.reserve(EVENT_CAPACITY);
        let limit = timeout.map(|timeout| Timespec {
            tv_sec: timeout.as_secs() as i64,
            tv_nsec: i64::from(timeout.subsec_nanos()),
        });
        match epoll::wait(
            &self.shared.epoll,
            rustix::buffer::spare_capacity(&mut events),
            limit.as_ref(),
        ) {
            Ok(_) => {}
            Err(rustix::io::Errno::INTR) => {}
            Err(error) => return Err(error.into()),
        }

        let mut timers_ready = false;
        for event in &events {
            let token = event.data.u64();
            if token == TIMER_TOKEN {
                timers_ready = true;
                continue;
            }
            self.dispatch(state, token, Ready::from_flags(event.flags));
        }
        self.shared.events.set(events);
        if timers_ready {
            let mut expirations = [0u8; 8];
            let _ = rustix::io::read(&self.shared.timerfd, &mut expirations);
            self.shared.inner.borrow_mut().armed = None;
        }
        self.expire(state);
        self.run_idle(state);
        self.run_woken(state);
        self.run_turn_hooks(state);
        let mut inner = self.shared.inner.borrow_mut();
        self.arm(&mut inner);
        Ok(())
    }

    fn insert(&self, source: Source<S>) -> u64 {
        self.shared.inner.borrow_mut().sources.insert(source)
    }

    fn dispatch(&self, state: &mut S, token: u64, ready: Ready) {
        let Some(callback_entry) = self.shared.inner.borrow_mut().sources.take(token) else {
            return;
        };
        let callback_entry = match callback_entry {
            TakenCallback::Watch(mut callback) => {
                callback(state, ready);
                TakenCallback::Watch(callback)
            }
            TakenCallback::Mailbox(notifier, mut drain) => {
                notifier.reset();
                drain(state);
                TakenCallback::Mailbox(notifier, drain)
            }
        };
        self.shared
            .inner
            .borrow_mut()
            .sources
            .restore(token, callback_entry);
        self.settle(state);
    }

    fn expire(&self, state: &mut S) {
        let now = Instant::now();
        loop {
            let due = {
                let mut inner = self.shared.inner.borrow_mut();
                let Inner {
                    sources, timers, ..
                } = &mut *inner;
                timers.pop_due(sources, now)
            };
            match due {
                Some(callback) => {
                    callback(state);
                    self.settle(state);
                }
                None => break,
            }
        }
    }

    fn run_idle(&self, state: &mut S) {
        let mut idle = self.shared.idle.take();
        std::mem::swap(&mut idle, &mut self.shared.inner.borrow_mut().idle);
        for callback in idle.drain(..) {
            callback(state);
            self.settle(state);
        }
        self.shared.idle.set(idle);
    }

    fn run_woken(&self, state: &mut S) {
        let mut woken = self.shared.hooks.take();
        woken.clear();
        std::mem::swap(&mut woken, &mut self.shared.inner.borrow_mut().woken);
        self.run_hooks(state, &woken);
        woken.clear();
        self.shared.hooks.set(woken);
    }

    fn run_turn_hooks(&self, state: &mut S) {
        let mut hooks = self.shared.hooks.take();
        self.shared.inner.borrow().sources.turn_hooks(&mut hooks);
        self.run_hooks(state, &hooks);
        self.shared.hooks.set(hooks);
    }

    fn run_hooks(&self, state: &mut S, tokens: &[u64]) {
        for &token in tokens {
            let Some(mut callback) = self.shared.inner.borrow_mut().sources.take_hook(token) else {
                continue;
            };
            callback(state);
            self.shared
                .inner
                .borrow_mut()
                .sources
                .restore_hook(token, callback);
            self.settle(state);
        }
    }

    pub fn settle(&self, state: &mut S) {
        let Some(mut settle) = self.shared.settle.take() else {
            return;
        };
        settle(state);
        if let Some(replaced) = self.shared.settle.replace(Some(settle)) {
            self.shared.settle.set(Some(replaced));
        }
    }

    fn arm(&self, inner: &mut Inner<S>) {
        let earliest = inner.timers.earliest(&inner.sources);
        if earliest == inner.armed {
            return;
        }
        inner.armed = earliest;
        let value = match earliest {
            Some(at) => {
                let delay = at
                    .saturating_duration_since(Instant::now())
                    .max(Duration::from_nanos(1));
                Timespec {
                    tv_sec: delay.as_secs() as i64,
                    tv_nsec: i64::from(delay.subsec_nanos()),
                }
            }
            None => Timespec {
                tv_sec: 0,
                tv_nsec: 0,
            },
        };
        let _ = timerfd_settime(
            &self.shared.timerfd,
            TimerfdTimerFlags::empty(),
            &Itimerspec {
                it_interval: Timespec {
                    tv_sec: 0,
                    tv_nsec: 0,
                },
                it_value: value,
            },
        );
    }
}
