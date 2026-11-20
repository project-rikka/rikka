use std::sync::Arc;

use crate::client::route::Destination;
use crate::ids::SERVER_ID_START;
use crate::interface::Interface;
use crate::key::Keys;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Life {
    Live,
    Departed,
    Destroyed,
}

pub(crate) struct Slot {
    pub(crate) generation: u32,
    pub(crate) interface: &'static Interface,
    pub(crate) version: u32,
    pub(crate) life: Life,
    pub(crate) route: Destination,
    pub(crate) keys: Option<Arc<Keys>>,
}

pub(crate) struct ProxyTable {
    client: Vec<Option<Slot>>,
    free: Vec<u32>,
    server: Vec<Option<Slot>>,
    generation: u32,
}

impl ProxyTable {
    pub(crate) fn new() -> Self {
        Self {
            client: vec![None],
            free: Vec::new(),
            server: Vec::new(),
            generation: 0,
        }
    }

    pub(crate) fn allocate(
        &mut self,
        interface: &'static Interface,
        version: u32,
        route: Destination,
    ) -> (u32, u32) {
        let slot = self.fresh(interface, version, route);
        let generation = slot.generation;
        let id = match self.free.pop() {
            Some(id) => {
                self.client[id as usize] = Some(slot);
                id
            }
            None => {
                self.client.push(Some(slot));
                (self.client.len() - 1) as u32
            }
        };
        (id, generation)
    }

    pub(crate) fn adopt(
        &mut self,
        id: u32,
        interface: &'static Interface,
        version: u32,
        route: Destination,
    ) -> Option<u32> {
        let index = id.checked_sub(SERVER_ID_START)? as usize;
        if index >= self.server.len() {
            self.server.resize_with(index + 1, || None);
        }
        let slot = self.fresh(interface, version, route);
        let generation = slot.generation;
        self.server[index] = Some(slot);
        Some(generation)
    }

    pub(crate) fn get(&self, id: u32) -> Option<&Slot> {
        self.entry(id)?.as_ref()
    }

    pub(crate) fn slot(&self, id: u32, generation: u32) -> Option<&Slot> {
        self.get(id).filter(|slot| slot.generation == generation)
    }

    pub(crate) fn slot_mut(&mut self, id: u32, generation: u32) -> Option<&mut Slot> {
        self.entry_mut(id)?
            .as_mut()
            .filter(|slot| slot.generation == generation)
    }

    pub(crate) fn get_mut(&mut self, id: u32) -> Option<&mut Slot> {
        self.entry_mut(id)?.as_mut()
    }

    pub(crate) fn release(&mut self, id: u32) {
        if id >= SERVER_ID_START {
            return;
        }
        if let Some(entry) = self.client.get_mut(id as usize)
            && entry.take().is_some()
        {
            self.free.push(id);
        }
    }

    fn fresh(&mut self, interface: &'static Interface, version: u32, route: Destination) -> Slot {
        self.generation = self.generation.wrapping_add(1);
        Slot {
            generation: self.generation,
            interface,
            version,
            life: Life::Live,
            route,
            keys: None,
        }
    }

    fn entry(&self, id: u32) -> Option<&Option<Slot>> {
        match id.checked_sub(SERVER_ID_START) {
            Some(index) => self.server.get(index as usize),
            None => self.client.get(id as usize),
        }
    }

    fn entry_mut(&mut self, id: u32) -> Option<&mut Option<Slot>> {
        match id.checked_sub(SERVER_ID_START) {
            Some(index) => self.server.get_mut(index as usize),
            None => self.client.get_mut(id as usize),
        }
    }
}
