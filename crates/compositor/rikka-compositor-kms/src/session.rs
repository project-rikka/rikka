use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::io;
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::path::Path;
use std::rc::Rc;

use libseat::{Seat, SeatEvent};
use rikka_compositor_reactor::{Interest, Reactor};

use crate::error::{Error, Result, seat_error};
use crate::opener::{DeviceOpener, DeviceRequest};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEvent {
    Paused,
    Resumed,
}

#[derive(Clone)]
pub struct Session {
    inner: Rc<Inner>,
}

struct Inner {
    seat: RefCell<Seat>,
    name: String,
    active: Cell<bool>,
    events: Rc<RefCell<VecDeque<SeatEvent>>>,
    devices: RefCell<HashMap<RawFd, libseat::Device>>,
}

impl Session {
    pub fn open<S: 'static>(
        reactor: &Reactor<S>,
        mut on: impl FnMut(&mut S, SessionEvent) + 'static,
    ) -> Result<Self> {
        let events = Rc::new(RefCell::new(VecDeque::new()));
        let queue = Rc::clone(&events);
        let mut seat = Seat::open(move |_, event| queue.borrow_mut().push_back(event))
            .map_err(|errno| Error::SeatUnavailable(seat_error(errno)))?;
        let name = seat.name().to_owned();
        seat.dispatch(0)
            .map_err(|errno| Error::SeatUnavailable(seat_error(errno)))?;
        let active = events
            .borrow_mut()
            .drain(..)
            .fold(false, |_, event| matches!(event, SeatEvent::Enable));
        let inner = Rc::new(Inner {
            seat: RefCell::new(seat),
            name,
            active: Cell::new(active),
            events,
            devices: RefCell::new(HashMap::new()),
        });
        let dispatched = Rc::clone(&inner);
        {
            let mut seat = inner.seat.borrow_mut();
            let fd = seat
                .get_fd()
                .map_err(|errno| Error::SeatUnavailable(seat_error(errno)))?;
            reactor
                .watch(fd, Interest::READABLE, move |state, _| {
                    dispatched.dispatch(state, &mut on);
                })
                .map_err(Error::Reactor)?;
        }
        tracing::info!(seat = inner.name, active, "session opened");
        Ok(Self { inner })
    }

    pub fn seat(&self) -> &str {
        &self.inner.name
    }

    pub fn is_active(&self) -> bool {
        self.inner.active.get()
    }

    pub fn open_device(&self, path: &Path) -> Result<OwnedFd> {
        let refused = |source: io::Error| Error::DeviceRefused {
            path: path.to_owned(),
            source,
        };
        let printable = path
            .to_str()
            .filter(|path| !path.contains('\0'))
            .ok_or_else(|| refused(io::Error::from(io::ErrorKind::InvalidInput)))?;
        let device = self
            .inner
            .seat
            .borrow_mut()
            .open_device(&printable)
            .map_err(|errno| refused(seat_error(errno)))?;
        let raw = device.as_fd().as_raw_fd();
        self.inner.devices.borrow_mut().insert(raw, device);
        Ok(unsafe { OwnedFd::from_raw_fd(raw) })
    }

    pub fn close_device(&self, fd: OwnedFd) -> Result<()> {
        let device = self.inner.devices.borrow_mut().remove(&fd.as_raw_fd());
        let closed = match device {
            Some(device) => self
                .inner
                .seat
                .borrow_mut()
                .close_device(device)
                .map_err(|errno| Error::DeviceNotClosed(seat_error(errno))),
            None => Ok(()),
        };
        drop(fd);
        closed
    }

    pub fn switch_vt(&self, vt: i32) -> Result<()> {
        self.inner
            .seat
            .borrow_mut()
            .switch_session(vt)
            .map_err(|errno| Error::VtSwitch {
                vt,
                source: seat_error(errno),
            })
    }

    pub fn opener<S: 'static>(&self, reactor: &Reactor<S>) -> Result<DeviceOpener> {
        let session = self.clone();
        let requests = reactor
            .mailbox(move |_: &mut S, request: DeviceRequest| session.serve(request))
            .map_err(Error::Reactor)?;
        Ok(DeviceOpener::new(requests))
    }

    fn serve(&self, request: DeviceRequest) {
        match request {
            DeviceRequest::Open { path, reply } => {
                let _ = reply.send(self.open_device(&path));
            }
            DeviceRequest::Close(fd) => {
                if let Err(error) = self.close_device(fd) {
                    tracing::warn!(%error, "failed to close device");
                }
            }
        }
    }
}

impl Inner {
    fn dispatch<S>(&self, state: &mut S, on: &mut impl FnMut(&mut S, SessionEvent)) {
        if let Err(errno) = self.seat.borrow_mut().dispatch(0) {
            tracing::warn!("failed to dispatch seat: {}", seat_error(errno));
        }
        loop {
            let Some(event) = self.events.borrow_mut().pop_front() else {
                break;
            };
            match event {
                SeatEvent::Disable => {
                    tracing::info!(seat = self.name, "session paused");
                    self.active.set(false);
                    on(state, SessionEvent::Paused);
                    if let Err(errno) = self.seat.borrow_mut().disable() {
                        tracing::warn!("failed to disable seat: {}", seat_error(errno));
                    }
                }
                SeatEvent::Enable => {
                    tracing::info!(seat = self.name, "session resumed");
                    self.active.set(true);
                    on(state, SessionEvent::Resumed);
                }
            }
        }
    }
}

impl fmt::Debug for Session {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Session")
            .field("seat", &self.inner.name)
            .field("active", &self.inner.active.get())
            .field("devices", &self.inner.devices.borrow().len())
            .finish()
    }
}
