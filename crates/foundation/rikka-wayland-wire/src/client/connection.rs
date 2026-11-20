use rikka_sync::{Condvar, Mutex, MutexGuard};
use std::fmt;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::Path;
use std::sync::{Arc, OnceLock, Weak};

use rustix::event::{PollFd, PollFlags, poll};
use rustix::io::Errno;

use crate::client::connect;
use crate::client::demux::{self, Arrival};
use crate::client::error::{Error, Result};
use crate::client::handle::Handle;
use crate::client::queue::{Queue, QueueShared};
use crate::client::request::Request;
use crate::client::route::Destination;
use crate::client::table::{Life, ProxyTable};
use crate::connection::{Connection as Socket, Fill};
use crate::ids::DISPLAY_ID;
use crate::interface::{ArgType, Interface, MessageSpec};
use crate::outgoing::{Flush, OUTGOING_LIMIT};

const DISPLAY_SYNC: u16 = 0;
const DISPLAY_VERSION: u32 = 1;

pub(crate) static BARRIER: Interface = Interface {
    index: u16::MAX,
    name: "wl_callback",
    version: 1,
    requests: &[],
    events: &[MessageSpec {
        name: "done",
        since: 1,
        destructor: true,
        signature: &[ArgType::Uint],
        children: &[],
    }],
};

pub(crate) struct Io {
    pub(crate) connection: Socket,
    pub(crate) table: ProxyTable,
}

struct Shared {
    socket: OwnedFd,
    io: Mutex<Io>,
    reading: Mutex<bool>,
    turned: Condvar,
    fault: OnceLock<Error>,
    queues: Mutex<Vec<Weak<QueueShared>>>,
}

#[derive(Clone)]
pub struct Connection(Arc<Shared>);

impl Connection {
    pub fn connect_to_env(display: &'static Interface) -> Result<(Self, Handle)> {
        Self::from_socket(connect::from_env()?, display)
    }

    pub fn connect_to(path: &Path, display: &'static Interface) -> Result<(Self, Handle)> {
        Self::from_socket(connect::to(path.to_owned())?, display)
    }

    pub fn from_socket(
        socket: impl Into<OwnedFd>,
        display: &'static Interface,
    ) -> Result<(Self, Handle)> {
        let socket = socket.into();
        let polled = socket.try_clone()?;
        let mut table = ProxyTable::new();
        let (id, generation) = table.allocate(display, DISPLAY_VERSION, Destination::Discard);
        let connection = Self(Arc::new(Shared {
            socket: polled,
            io: Mutex::new(Io {
                connection: Socket::new(socket),
                table,
            }),
            reading: Mutex::new(false),
            turned: Condvar::new(),
            fault: OnceLock::new(),
            queues: Mutex::new(Vec::new()),
        }));
        let handle = Handle::new(&connection, id, generation, display, DISPLAY_VERSION);
        Ok((connection, handle))
    }

    pub fn queue<S>(&self) -> Queue<S> {
        let queue = Queue::new(self);
        let mut queues = self.0.queues.lock();
        queues.retain(|known| known.strong_count() > 0);
        queues.push(queue.downgrade());
        queue
    }

    pub fn error(&self) -> Option<Error> {
        self.0.fault.get().cloned()
    }

    pub fn flush(&self) -> Result<()> {
        loop {
            let flushed = self.io().connection.flush();
            match flushed {
                Ok(Flush::Done) => return Ok(()),
                Ok(Flush::Pending) => self.poll(PollFlags::OUT)?,
                Err(error) => return self.flush_failed(error.into()),
            }
        }
    }

    pub fn try_flush(&self) -> Result<bool> {
        let flushed = self.io().connection.flush();
        match flushed {
            Ok(Flush::Done) => Ok(true),
            Ok(Flush::Pending) => Ok(false),
            Err(error) => self.flush_failed(error.into()).map(|()| true),
        }
    }

    pub fn read(&self) -> Result<usize> {
        let mut reading = self.0.reading.lock();
        while *reading {
            reading = self.0.turned.wait(reading);
        }
        *reading = true;
        drop(reading);
        let outcome = self.pump();
        *self.0.reading.lock() = false;
        self.0.turned.notify_all();
        outcome
    }

    pub(crate) fn wait(&self, ready: impl Fn() -> bool) -> Result<()> {
        let mut reading = self.0.reading.lock();
        loop {
            if ready() {
                return Ok(());
            }
            self.check()?;
            if *reading {
                reading = self.0.turned.wait(reading);
                continue;
            }
            *reading = true;
            drop(reading);
            let outcome = self.poll(PollFlags::IN).and_then(|()| self.pump());
            reading = self.0.reading.lock();
            *reading = false;
            self.0.turned.notify_all();
            outcome?;
        }
    }

    pub(crate) fn check(&self) -> Result<()> {
        match self.0.fault.get() {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }

    pub(crate) fn io(&self) -> MutexGuard<'_, Io> {
        self.0.io.lock()
    }

    pub(crate) fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub(crate) fn inert(&self, interface: &'static Interface) -> Handle {
        Handle::new(self, 0, 0, interface, 0)
    }

    pub(crate) fn barrier(&self, queue: &Arc<QueueShared>) -> Result<()> {
        self.check()?;
        let mut io = self.io();
        let route = Destination::Queue(Arc::clone(queue));
        let barrier = io.table.allocate(&BARRIER, 1, route);
        let mut writer = io.connection.write(DISPLAY_ID, DISPLAY_SYNC);
        writer.uint(barrier.0);
        let written = writer.finish();
        drop(io);
        match written {
            Ok(()) => Ok(()),
            Err(error) => Err(self.fail(error.into())),
        }
    }

    pub(crate) fn send(
        &self,
        request: Request<'_>,
        created: Option<&'static Interface>,
    ) -> Option<Handle> {
        let Request {
            handle,
            opcode,
            mut body,
            fds,
            objects,
            creation,
            destructor,
            broken,
        } = request;
        let spec = handle.interface().requests.get(usize::from(opcode))?;
        if broken || handle.version() < spec.since {
            tracing::debug!("dropped {handle:?} request {}", spec.name);
            return None;
        }
        let mut io = self.io();
        let live = |io: &Io, id: u32, generation: u32| {
            io.table
                .slot(id, generation)
                .is_some_and(|slot| slot.life == Life::Live)
        };
        if !live(&io, handle.id(), handle.generation())
            || !objects
                .iter()
                .all(|(id, generation)| live(&io, *id, *generation))
        {
            tracing::debug!(
                "dropped {handle:?} request {} naming a dead object",
                spec.name
            );
            return None;
        }
        let inherited = io
            .table
            .slot(handle.id(), handle.generation())?
            .route
            .clone();
        let child = match (created, creation) {
            (Some(interface), Some(creation)) => {
                let version = creation.version.unwrap_or(handle.version());
                let route = handle
                    .children()
                    .map_or(inherited.clone(), |route| route.0.clone());
                let (id, generation) = io.table.allocate(interface, version, route);
                body[creation.offset..creation.offset + 4].copy_from_slice(&id.to_ne_bytes());
                Some(Handle::new(self, id, generation, interface, version))
            }
            _ => None,
        };
        if io.connection.pending_output() > OUTGOING_LIMIT / 2 {
            self.drain(&io);
        }
        let mut writer = io.connection.write(handle.id(), opcode);
        writer.raw(&body);
        for fd in fds {
            writer.fd(fd);
        }
        let written = writer.finish();
        if destructor && let Some(slot) = io.table.slot_mut(handle.id(), handle.generation()) {
            slot.life = Life::Destroyed;
            if let Destination::Queue(queue) = &inherited {
                queue.forget(handle.id(), handle.generation());
            }
        }
        drop(io);
        if let Err(error) = written {
            self.fail(error.into());
        }
        child
    }

    fn drain(&self, io: &Io) {
        while let Ok(Flush::Pending) = io.connection.flush() {
            if self.poll(PollFlags::OUT).is_err() {
                return;
            }
        }
    }

    fn pump(&self) -> Result<usize> {
        let mut arrivals = Vec::new();
        let outcome = self.collect(&mut arrivals);
        let delivered = arrivals.len();
        for (route, packet) in arrivals {
            match route {
                Destination::Queue(queue) => queue.push(packet),
                Destination::Tap(tap) => tap(packet),
                Destination::Discard => {}
            }
        }
        match outcome {
            Ok(()) => Ok(delivered),
            Err(error) => Err(self.fail(error)),
        }
    }

    fn collect(
        &self,
        arrivals: &mut Vec<(Destination, crate::client::packet::Packet)>,
    ) -> Result<()> {
        let mut io = self.io();
        loop {
            let filled = io.connection.fill()?;
            while let Some(arrival) = demux::next(self, &mut io)? {
                match arrival {
                    Arrival::Deliver(route, packet) => arrivals.push((route, packet)),
                    Arrival::Fault(error) => return Err(error),
                    Arrival::Nothing => {}
                }
            }
            match filled {
                Fill::Read(_) | Fill::Full => {}
                Fill::WouldBlock => return Ok(()),
                Fill::Closed => return Err(Error::Closed),
            }
        }
    }

    fn poll(&self, flags: PollFlags) -> Result<()> {
        loop {
            let mut polled = [PollFd::new(&self.0.socket, flags)];
            match poll(&mut polled, None) {
                Ok(_) => return Ok(()),
                Err(Errno::INTR) => {}
                Err(error) => return Err(self.fail(error.into())),
            }
        }
    }

    fn flush_failed(&self, error: Error) -> Result<()> {
        let hung_up = matches!(
            &error,
            Error::Io(io) if matches!(
                io.kind(),
                std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
            )
        );
        match hung_up {
            true => Ok(()),
            false => Err(self.fail(error)),
        }
    }

    fn fail(&self, error: Error) -> Error {
        let fault = self.0.fault.get_or_init(|| error).clone();
        for queue in self.0.queues.lock().iter().filter_map(Weak::upgrade) {
            queue.wake();
        }
        fault
    }
}

impl AsFd for Connection {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.socket.as_fd()
    }
}

impl fmt::Debug for Connection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Connection")
            .field("socket", &self.0.socket)
            .field("failed", &self.0.fault.get().is_some())
            .finish()
    }
}
