use rikka_sync::{Mutex, MutexGuard};
use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;
use std::task::Waker;

use crate::client::connection::{BARRIER, Connection};
use crate::client::error::Result;
use crate::client::packet::Packet;
use crate::client::proxy::Proxy;
use crate::client::route::{Destination, Route};

type Handler<S> = Box<dyn Fn(&mut S, Packet) + Send>;

#[derive(Default)]
pub(crate) struct QueueShared {
    packets: Mutex<VecDeque<Packet>>,
    waker: Mutex<Option<Waker>>,
}

impl QueueShared {
    pub(crate) fn push(&self, packet: Packet) {
        let first = {
            let mut packets = self.packets();
            packets.push_back(packet);
            packets.len() == 1
        };
        if first && let Some(waker) = self.waker.lock().as_ref() {
            waker.wake_by_ref();
        }
    }

    pub(crate) fn wake(&self) {
        if let Some(waker) = self.waker.lock().as_ref() {
            waker.wake_by_ref();
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.packets().is_empty()
    }

    pub(crate) fn forget(&self, id: u32, generation: u32) {
        self.packets()
            .retain(|packet| packet.handle.id() != id || packet.handle.generation() != generation);
    }

    fn pop(&self) -> Option<Packet> {
        self.packets().pop_front()
    }

    fn packets(&self) -> MutexGuard<'_, VecDeque<Packet>> {
        self.packets.lock()
    }
}

pub struct Queue<S> {
    connection: Connection,
    shared: Arc<QueueShared>,
    handlers: Vec<Option<Handler<S>>>,
    barriers: Barriers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Barrier(u64);

#[derive(Default)]
struct Barriers {
    sent: u64,
    passed: u64,
}

impl<S> Queue<S> {
    pub(crate) fn new(connection: &Connection) -> Self {
        Self {
            connection: connection.clone(),
            shared: Arc::new(QueueShared::default()),
            handlers: Vec::new(),
            barriers: Barriers::default(),
        }
    }

    pub(crate) fn downgrade(&self) -> std::sync::Weak<QueueShared> {
        Arc::downgrade(&self.shared)
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    pub fn route(&self) -> Route {
        Route(Destination::Queue(Arc::clone(&self.shared)))
    }

    pub fn set_waker(&self, waker: Waker) {
        *self.shared.waker.lock() = Some(waker);
    }

    pub fn on<P: Proxy>(&mut self, handler: impl Fn(&mut S, P, P::Event) + Send + 'static) {
        let index = usize::from(P::INTERFACE.index);
        if self.handlers.len() <= index {
            self.handlers.resize_with(index + 1, || None);
        }
        self.handlers[index] = Some(Box::new(move |state: &mut S, packet: Packet| {
            packet.deliver::<P>(|proxy, event| handler(state, proxy, event));
        }));
    }

    pub fn has_pending(&self) -> bool {
        !self.shared.is_empty()
    }

    pub fn dispatch_pending(&mut self, state: &mut S) -> Result<usize> {
        let mut dispatched = 0;
        while let Some(packet) = self.shared.pop() {
            dispatched += 1;
            let interface = packet.handle.interface();
            if std::ptr::eq(interface, &BARRIER) {
                self.barriers.passed += 1;
                continue;
            }
            if let Some(Some(handler)) = self.handlers.get(usize::from(interface.index)) {
                handler(state, packet);
            }
        }
        match dispatched {
            0 => self.connection.check().map(|()| 0),
            dispatched => Ok(dispatched),
        }
    }

    pub fn blocking_dispatch(&mut self, state: &mut S) -> Result<usize> {
        self.connection.flush()?;
        let shared = Arc::clone(&self.shared);
        self.connection.wait(|| !shared.is_empty())?;
        self.dispatch_pending(state)
    }

    pub fn sync(&mut self) -> Result<Barrier> {
        self.connection.barrier(&self.shared)?;
        self.barriers.sent += 1;
        Ok(Barrier(self.barriers.sent))
    }

    pub fn has_passed(&self, barrier: Barrier) -> bool {
        self.barriers.passed >= barrier.0
    }

    pub fn roundtrip(&mut self, state: &mut S) -> Result<()> {
        let barrier = self.sync()?;
        loop {
            self.dispatch_pending(state)?;
            if self.has_passed(barrier) {
                return Ok(());
            }
            self.connection.flush()?;
            let shared = Arc::clone(&self.shared);
            self.connection.wait(|| !shared.is_empty())?;
        }
    }
}

impl<S> Drop for Queue<S> {
    fn drop(&mut self) {
        self.shared.packets().clear();
    }
}

impl<S> fmt::Debug for Queue<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Queue")
            .field("pending", &self.shared.packets().len())
            .finish_non_exhaustive()
    }
}
