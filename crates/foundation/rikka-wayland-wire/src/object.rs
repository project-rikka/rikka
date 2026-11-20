use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use crate::event::EventWriter;
use crate::ids::ClientId;
use crate::interface::Interface;
use crate::key::Key;
use crate::peer::Peer;

#[derive(Clone)]
pub struct Object {
    peer: Rc<Peer>,
    interface: &'static Interface,
    client: ClientId,
    id: u32,
    generation: u32,
    version: u32,
}

impl Object {
    pub(crate) fn new(
        peer: &Rc<Peer>,
        id: u32,
        generation: u32,
        interface: &'static Interface,
        version: u32,
    ) -> Self {
        Self {
            peer: peer.clone(),
            interface,
            client: peer.id(),
            id,
            generation,
            version,
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }

    pub fn interface(&self) -> &'static Interface {
        self.interface
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn client(&self) -> ClientId {
        self.client
    }

    pub fn peer(&self) -> &Rc<Peer> {
        &self.peer
    }

    pub fn alive(&self) -> bool {
        self.peer.is_open()
            && self
                .peer
                .table()
                .slot(self.id, self.generation)
                .is_some_and(|slot| slot.alive)
    }

    pub fn expire(&self) {
        if let Some(slot) = self.peer.table_mut().slot_mut(self.id, self.generation) {
            slot.alive = false;
        }
    }

    pub fn set_key<K: Key>(&self, key: K) {
        if let Some(slot) = self.peer.table_mut().slot_mut(self.id, self.generation) {
            slot.keys.set(key);
        }
    }

    pub fn key<K: Key>(&self) -> Option<K> {
        self.peer
            .table()
            .slot(self.id, self.generation)?
            .keys
            .get::<K>()
    }

    pub fn take_key<K: Key>(&self) -> Option<K> {
        self.peer
            .table_mut()
            .slot_mut(self.id, self.generation)?
            .keys
            .take::<K>()
    }

    #[inline]
    pub fn event(&self, opcode: u16) -> Option<EventWriter<'_>> {
        EventWriter::begin(self, opcode)
    }

    pub fn post_error(&self, code: u32, message: &str) {
        self.peer
            .post_error(self.interface.name, self.id, code, message);
    }
}

impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        self.client == other.client && self.id == other.id && self.generation == other.generation
    }
}

impl Eq for Object {}

impl Hash for Object {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.client.hash(state);
        self.id.hash(state);
        self.generation.hash(state);
    }
}

impl fmt::Debug for Object {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}#{}.{} (client {}, v{})",
            self.interface.name,
            self.id,
            self.generation,
            self.client.raw(),
            self.version,
        )
    }
}
