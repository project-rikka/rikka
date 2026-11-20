use std::cell::RefMut;
use std::rc::Rc;

use crate::error::{Result, WireError};
use crate::ids::{IdPool, SERVER_ID_START, is_client_id};
use crate::interface::Interface;
use crate::key::Keys;
use crate::object::Object;
use crate::peer::Peer;
use crate::resource::Resource;

const MAX_CLIENT_OBJECTS: u32 = 0x00F0_0000;

#[derive(Debug)]
pub(crate) struct Slot {
    pub(crate) generation: u32,
    pub(crate) interface: &'static Interface,
    pub(crate) version: u32,
    pub(crate) alive: bool,
    pub(crate) retired: bool,
    pub(crate) keys: Keys,
}

#[derive(Debug)]
pub(crate) struct ObjectTable {
    client: Vec<Option<Slot>>,
    server: Vec<Option<Slot>>,
    pool: IdPool,
    generation: u32,
}

impl ObjectTable {
    pub(crate) fn new() -> Self {
        Self {
            client: vec![None],
            server: Vec::new(),
            pool: IdPool::new(),
            generation: 0,
        }
    }

    pub(crate) fn next_generation(&mut self) -> u32 {
        self.generation = self.generation.wrapping_add(1);
        self.generation
    }

    pub(crate) fn insert_client(
        &mut self,
        id: u32,
        interface: &'static Interface,
        version: u32,
    ) -> Result<u32> {
        if !is_client_id(id) || id > MAX_CLIENT_OBJECTS {
            return Err(WireError::InvalidNewId(id));
        }
        let index = id as usize;
        match index.cmp(&self.client.len()) {
            std::cmp::Ordering::Greater => return Err(WireError::InvalidNewId(id)),
            std::cmp::Ordering::Equal => self.client.push(None),
            std::cmp::Ordering::Less if self.client[index].is_some() => {
                return Err(WireError::InvalidNewId(id));
            }
            std::cmp::Ordering::Less => {}
        }
        let generation = self.next_generation();
        self.client[index] = Some(Slot::new(generation, interface, version));
        Ok(generation)
    }

    pub(crate) fn insert_server(
        &mut self,
        interface: &'static Interface,
        version: u32,
    ) -> Option<(u32, u32)> {
        let id = self.pool.allocate()?;
        let index = (id - SERVER_ID_START) as usize;
        if index == self.server.len() {
            self.server.push(None);
        }
        let generation = self.next_generation();
        self.server[index] = Some(Slot::new(generation, interface, version));
        Some((id, generation))
    }

    pub(crate) fn live(&self, id: u32) -> Option<&Slot> {
        self.entry(id)?.as_ref().filter(|slot| slot.alive)
    }

    pub(crate) fn slot(&self, id: u32, generation: u32) -> Option<&Slot> {
        self.entry(id)?
            .as_ref()
            .filter(|slot| slot.generation == generation)
    }

    pub(crate) fn slot_mut(&mut self, id: u32, generation: u32) -> Option<&mut Slot> {
        self.entry_mut(id)?
            .as_mut()
            .filter(|slot| slot.generation == generation)
    }

    pub(crate) fn retire(&mut self, id: u32, generation: u32) -> bool {
        match self.slot_mut(id, generation) {
            Some(slot) if !slot.retired => {
                slot.alive = false;
                slot.retired = true;
                true
            }
            _ => false,
        }
    }

    pub(crate) fn free(&mut self, id: u32, generation: u32) {
        let Some(entry) = self.entry_mut(id) else {
            return;
        };
        if entry
            .as_ref()
            .is_some_and(|slot| slot.generation == generation)
        {
            *entry = None;
            if id >= SERVER_ID_START {
                self.pool.release(id);
            }
        }
    }

    pub(crate) fn retire_all(&mut self) -> Vec<(u32, &Slot)> {
        let client = self
            .client
            .iter_mut()
            .enumerate()
            .map(|(index, slot)| (index as u32, slot));
        let server = self
            .server
            .iter_mut()
            .enumerate()
            .map(|(index, slot)| (SERVER_ID_START + index as u32, slot));
        client
            .chain(server)
            .filter_map(|(id, slot)| {
                let slot = slot.as_mut()?;
                slot.alive = false;
                slot.retired = true;
                Some((id, &*slot))
            })
            .collect()
    }

    pub(crate) fn clear(&mut self) {
        self.client.clear();
        self.server.clear();
    }

    fn entry(&self, id: u32) -> Option<&Option<Slot>> {
        if id >= SERVER_ID_START {
            self.server.get((id - SERVER_ID_START) as usize)
        } else {
            self.client.get(id as usize)
        }
    }

    fn entry_mut(&mut self, id: u32) -> Option<&mut Option<Slot>> {
        if id >= SERVER_ID_START {
            self.server.get_mut((id - SERVER_ID_START) as usize)
        } else {
            self.client.get_mut(id as usize)
        }
    }
}

impl Slot {
    fn new(generation: u32, interface: &'static Interface, version: u32) -> Self {
        Self {
            generation,
            interface,
            version,
            alive: true,
            retired: false,
            keys: Keys::default(),
        }
    }
}

pub struct Objects<'p> {
    peer: &'p Rc<Peer>,
    table: RefMut<'p, ObjectTable>,
}

impl<'p> Objects<'p> {
    pub(crate) fn new(peer: &'p Rc<Peer>, table: RefMut<'p, ObjectTable>) -> Self {
        Self { peer, table }
    }

    pub fn get<R: Resource>(&self, id: u32) -> Result<R> {
        let object = self.find(id, Some(R::INTERFACE))?;
        Ok(R::from_object(object).expect("an object of R's interface converts to R"))
    }

    pub fn nullable<R: Resource>(&self, id: Option<u32>) -> Result<Option<R>> {
        id.map(|id| self.get(id)).transpose()
    }

    pub fn any(&self, id: u32) -> Result<Object> {
        self.find(id, None)
    }

    pub fn nullable_any(&self, id: Option<u32>) -> Result<Option<Object>> {
        id.map(|id| self.any(id)).transpose()
    }

    pub fn create<R: Resource>(&mut self, id: u32, version: u32) -> Result<R> {
        let generation = self.table.insert_client(id, R::INTERFACE, version)?;
        let object = Object::new(self.peer, id, generation, R::INTERFACE, version);
        Ok(R::from_object(object).expect("an object created for R has R's interface"))
    }

    fn find(&self, id: u32, interface: Option<&'static Interface>) -> Result<Object> {
        let slot = self.table.live(id).ok_or(WireError::UnknownObject(id))?;
        if let Some(expected) = interface
            && !slot.interface.is(expected)
        {
            return Err(WireError::WrongInterface {
                id,
                found: slot.interface.name,
                expected: expected.name,
            });
        }
        Ok(Object::new(
            self.peer,
            id,
            slot.generation,
            slot.interface,
            slot.version,
        ))
    }
}

impl std::fmt::Debug for Objects<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Objects")
            .field("client", &self.peer.id())
            .finish_non_exhaustive()
    }
}
