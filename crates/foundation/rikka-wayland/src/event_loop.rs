use std::ops::ControlFlow;
use std::os::fd::{AsFd as _, BorrowedFd};
use std::sync::Arc;
use std::task::Waker;

use rikka_wayland_client::{Connection, Proxy, Queue, Route};
use rikka_worker::CommandReceiver;

use crate::error::{Error, Result};

pub struct EventLoop<S> {
    connection: Connection,
    queue: Queue<S>,
}

impl<S> EventLoop<S> {
    pub(crate) fn new(connection: &Connection) -> EventLoop<S> {
        EventLoop {
            connection: connection.clone(),
            queue: connection.queue(),
        }
    }

    #[must_use]
    pub fn route(&self) -> Route {
        self.queue.route()
    }

    pub fn queue_mut(&mut self) -> &mut Queue<S> {
        &mut self.queue
    }

    pub fn on<P: Proxy>(&mut self, handler: impl Fn(&mut S, P, P::Event) + Send + 'static) {
        self.queue.on(handler);
    }

    #[must_use]
    pub fn as_fd(&self) -> BorrowedFd<'_> {
        self.connection.as_fd()
    }

    pub fn flush(&mut self) -> Result<()> {
        self.connection.flush().map_err(Error::from)
    }

    pub fn roundtrip(&mut self, state: &mut S) -> Result<()> {
        self.queue.roundtrip(state).map_err(Error::from)
    }

    pub fn dispatch_pending(&mut self, state: &mut S) -> Result<usize> {
        self.queue.dispatch_pending(state).map_err(Error::from)
    }

    pub fn blocking_dispatch(
        &mut self,
        state: &mut S,
        wait: impl FnOnce(BorrowedFd<'_>) -> std::io::Result<bool>,
    ) -> Result<bool> {
        if self.dispatch_pending(state)? > 0 {
            return Ok(true);
        }
        self.flush()?;
        if !wait(self.connection.as_fd())? {
            return Ok(false);
        }
        self.connection.read()?;
        self.dispatch_pending(state)?;
        Ok(true)
    }

    pub fn run<C>(
        &mut self,
        state: &mut S,
        commands: &CommandReceiver<C>,
        mut handle: impl FnMut(&mut S, C) -> ControlFlow<()>,
    ) -> Result<()> {
        self.queue.set_waker(commands.waker());
        let caught_up = self.queue.sync()?;
        loop {
            if commands.is_stopped() {
                return self.flush();
            }
            self.dispatch_pending(state)?;
            if self.queue.has_passed(caught_up) {
                for command in commands.drain() {
                    if handle(state, command).is_break() {
                        return self.flush();
                    }
                }
            }
            self.flush()?;
            commands.wait()?;
        }
    }

    pub fn settle(&mut self, state: &mut S) -> Result<()> {
        let (notifier, woken) = rikka_worker::event_fd()?;
        self.queue.set_waker(Waker::from(Arc::new(notifier)));
        let settled = self.queue.sync()?;
        self.flush()?;
        loop {
            self.dispatch_pending(state)?;
            if self.queue.has_passed(settled) {
                return Ok(());
            }
            woken.wait()?;
        }
    }

    pub fn poll_dispatch(&mut self, state: &mut S, waker: &Waker) -> Result<()> {
        self.queue.set_waker(waker.clone());
        self.dispatch_pending(state)?;
        self.flush()
    }
}

impl<S> std::fmt::Debug for EventLoop<S> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("EventLoop").finish_non_exhaustive()
    }
}
