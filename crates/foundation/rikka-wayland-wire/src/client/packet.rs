use std::collections::VecDeque;
use std::fmt;
use std::os::fd::OwnedFd;

use smallvec::SmallVec;

use crate::client::connection::Connection;
use crate::client::handle::Handle;
use crate::client::proxy::Proxy;
use crate::error::{Result, WireError};
use crate::interface::Interface;
use crate::message::Args;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Entry {
    pub(crate) id: u32,
    pub(crate) generation: u32,
    pub(crate) interface: &'static Interface,
    pub(crate) version: u32,
}

pub(crate) struct Packet {
    pub(crate) handle: Handle,
    pub(crate) opcode: u16,
    pub(crate) body: Box<[u8]>,
    pub(crate) fds: VecDeque<OwnedFd>,
    pub(crate) entries: SmallVec<[Entry; 2]>,
}

impl Packet {
    pub(crate) fn deliver<P: Proxy>(mut self, to: impl FnOnce(P, P::Event)) {
        let handle = self.handle.clone();
        let Some(spec) = handle.interface().events.get(usize::from(self.opcode)) else {
            return;
        };
        let Some(proxy) = P::from_handle(handle.clone()) else {
            return;
        };
        let mut args = Args::new(&self.body, spec.signature, &mut self.fds);
        let mut objects = Resolved {
            connection: handle.connection(),
            entries: &self.entries,
        };
        let decoded = P::decode(&handle, self.opcode, &mut args, &mut objects)
            .and_then(|event| args.finish().map(|()| event));
        match decoded {
            Ok(event) => to(proxy, event),
            Err(error) => tracing::warn!("dropped {handle:?} event {}: {error}", spec.name),
        }
    }
}

pub struct Resolved<'p> {
    connection: &'p Connection,
    entries: &'p [Entry],
}

impl Resolved<'_> {
    pub fn get<P: Proxy>(&self, id: u32) -> Result<P> {
        let handle = self.any(id)?;
        let found = handle.interface().name;
        P::from_handle(handle).ok_or(WireError::WrongInterface {
            id,
            found,
            expected: P::INTERFACE.name,
        })
    }

    pub fn nullable<P: Proxy>(&self, id: Option<u32>) -> Result<Option<P>> {
        id.map(|id| self.get(id)).transpose()
    }

    pub fn any(&self, id: u32) -> Result<Handle> {
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .ok_or(WireError::UnknownObject(id))?;
        Ok(Handle::new(
            self.connection,
            entry.id,
            entry.generation,
            entry.interface,
            entry.version,
        ))
    }

    pub fn nullable_any(&self, id: Option<u32>) -> Result<Option<Handle>> {
        id.map(|id| self.any(id)).transpose()
    }

    pub fn create<P: Proxy>(&self, id: u32, _version: u32) -> Result<P> {
        self.get(id)
    }
}

impl fmt::Debug for Resolved<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Resolved")
            .field("objects", &self.entries.len())
            .finish()
    }
}
