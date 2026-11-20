use rikka_physics_core::IdPool;

use crate::body::NONE;

#[derive(Clone, Copy, Debug)]
pub(crate) struct IslandList {
    pub head: u32,
    pub tail: u32,
    pub count: u32,
}

impl IslandList {
    pub(crate) const EMPTY: Self = Self {
        head: NONE,
        tail: NONE,
        count: 0,
    };
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Island {
    pub set: u32,
    pub local: u32,
    pub bodies: IslandList,
    pub contacts: IslandList,
    pub joints: IslandList,
    pub constraint_removed: u32,
    pub dirty: bool,
    pub alive: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Islands {
    pub records: Vec<Island>,
    ids: IdPool,
}

impl Islands {
    pub(crate) fn allocate(&mut self, set: u32, local: u32) -> u32 {
        let id = self.ids.alloc();
        let island = Island {
            set,
            local,
            bodies: IslandList::EMPTY,
            contacts: IslandList::EMPTY,
            joints: IslandList::EMPTY,
            constraint_removed: 0,
            dirty: true,
            alive: true,
        };
        if id as usize == self.records.len() {
            self.records.push(island);
        } else {
            self.records[id as usize] = island;
        }
        id
    }

    pub(crate) fn free(&mut self, id: u32) {
        self.records[id as usize].alive = false;
        self.ids.free(id);
    }

    #[inline]
    pub(crate) fn get(&self, id: u32) -> &Island {
        &self.records[id as usize]
    }

    #[inline]
    pub(crate) fn get_mut(&mut self, id: u32) -> &mut Island {
        &mut self.records[id as usize]
    }
}
