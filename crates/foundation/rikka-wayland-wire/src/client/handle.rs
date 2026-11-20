use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::client::connection::Connection;
use crate::client::object_id::ObjectId;
use crate::client::request::Request;
use crate::client::route::Route;
use crate::client::table::Life;
use crate::interface::Interface;
use crate::key::{Key, Keys};

#[derive(Clone)]
pub struct Handle {
    connection: Connection,
    interface: &'static Interface,
    id: u32,
    generation: u32,
    version: u32,
    children: Option<Route>,
    keys: Option<Arc<Keys>>,
}

impl Handle {
    pub(crate) fn new(
        connection: &Connection,
        id: u32,
        generation: u32,
        interface: &'static Interface,
        version: u32,
    ) -> Self {
        Self {
            connection: connection.clone(),
            interface,
            id,
            generation,
            version,
            children: None,
            keys: None,
        }
    }

    pub(crate) fn keyed(mut self, keys: Option<&Arc<Keys>>) -> Self {
        self.keys = keys.cloned();
        self
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn object_id(&self) -> ObjectId {
        ObjectId::new(self.id, self.generation)
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn interface(&self) -> &'static Interface {
        self.interface
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    pub fn alive(&self) -> bool {
        self.connection
            .io()
            .table
            .slot(self.id, self.generation)
            .is_some_and(|slot| slot.life == Life::Live)
    }

    pub fn set_key<K: Key>(&self, key: K) {
        if let Some(slot) = self
            .connection
            .io()
            .table
            .slot_mut(self.id, self.generation)
        {
            Arc::make_mut(slot.keys.get_or_insert_default()).set(key);
        }
    }

    pub fn key<K: Key>(&self) -> Option<K> {
        let io = self.connection.io();
        let keys = match io.table.slot(self.id, self.generation) {
            Some(slot) => slot.keys.as_ref(),
            None => self.keys.as_ref(),
        };
        keys?.get::<K>()
    }

    pub fn take_key<K: Key>(&self) -> Option<K> {
        let mut io = self.connection.io();
        let keys = io.table.slot_mut(self.id, self.generation)?.keys.as_mut()?;
        Arc::make_mut(keys).take::<K>()
    }

    #[must_use]
    pub fn via(&self, route: &Route) -> Self {
        Self {
            children: Some(route.clone()),
            ..self.clone()
        }
    }

    pub fn request(&self, opcode: u16) -> Request<'_> {
        Request::new(self, opcode)
    }

    pub(crate) fn generation(&self) -> u32 {
        self.generation
    }

    pub(crate) fn children(&self) -> Option<&Route> {
        self.children.as_ref()
    }
}

impl PartialEq for Handle {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.generation == other.generation
            && self.connection.same(&other.connection)
    }
}

impl Eq for Handle {}

impl Hash for Handle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.generation.hash(state);
    }
}

impl fmt::Debug for Handle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}#{}.{} (v{})",
            self.interface.name, self.id, self.generation, self.version,
        )
    }
}
