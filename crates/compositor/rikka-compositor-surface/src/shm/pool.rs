use std::cell::Cell;
use std::io;
use std::os::fd::{AsFd, OwnedFd};

use slotmap::SlotMap;

use super::mapping::Mapping;
use crate::ids::PoolId;

#[derive(Debug)]
pub(crate) struct Pool {
    mapping: Mapping,
    buffers: u32,
    open: bool,
    truncated: Cell<bool>,
}

impl Pool {
    pub(crate) fn range(&self) -> (*const u8, usize) {
        (self.mapping.base(), self.mapping.len())
    }

    pub(crate) fn truncated(&self) -> bool {
        self.truncated.get()
    }

    pub(crate) fn writable(&self) -> bool {
        self.mapping.writable()
    }

    pub(crate) fn mark_truncated(&self) {
        self.truncated.set(true);
    }
}

#[derive(Debug)]
pub(crate) enum GrowError {
    Shrink,
    Map(io::Error),
}

#[derive(Debug, Default)]
pub(crate) struct Pools {
    pools: SlotMap<PoolId, Pool>,
}

impl Pools {
    pub(crate) fn create(&mut self, fd: OwnedFd, len: usize) -> io::Result<PoolId> {
        let mapping = Mapping::new(fd.as_fd(), len)?;
        Ok(self.pools.insert(Pool {
            mapping,
            buffers: 0,
            open: true,
            truncated: Cell::new(false),
        }))
    }

    pub(crate) fn get(&self, pool: PoolId) -> Option<&Pool> {
        self.pools.get(pool)
    }

    pub(crate) fn len(&self, pool: PoolId) -> Option<usize> {
        self.pools.get(pool).map(|pool| pool.mapping.len())
    }

    pub(crate) fn grow(&mut self, pool: PoolId, len: usize) -> Result<(), GrowError> {
        let Some(pool) = self.pools.get_mut(pool) else {
            return Ok(());
        };
        if len < pool.mapping.len() {
            return Err(GrowError::Shrink);
        }
        if len == pool.mapping.len() || pool.truncated.get() {
            return Ok(());
        }
        pool.mapping.grow(len).map_err(GrowError::Map)
    }

    pub(crate) fn retain(&mut self, pool: PoolId) {
        if let Some(pool) = self.pools.get_mut(pool) {
            pool.buffers += 1;
        }
    }

    pub(crate) fn release(&mut self, pool: PoolId) {
        let Some(entry) = self.pools.get_mut(pool) else {
            return;
        };
        entry.buffers -= 1;
        if entry.buffers == 0 && !entry.open {
            self.pools.remove(pool);
        }
    }

    pub(crate) fn close(&mut self, pool: PoolId) {
        let Some(entry) = self.pools.get_mut(pool) else {
            return;
        };
        entry.open = false;
        if entry.buffers == 0 {
            self.pools.remove(pool);
        }
    }
}
