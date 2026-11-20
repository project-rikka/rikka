use std::fmt;
use std::os::fd::OwnedFd;

use slotmap::SlotMap;

use crate::ids::{BlockerId, SurfaceId};
use crate::syncobj::TimelinePoint;
use crate::waits::Waits;

pub trait Blocker: fmt::Debug + 'static {}

#[derive(Debug)]
pub(crate) struct FdWait;

impl Blocker for FdWait {}

#[derive(Debug, Default)]
pub(crate) struct Blockers {
    active: SlotMap<BlockerId, Box<dyn Blocker>>,
    pub(crate) waits: Waits,
}

impl Blockers {
    pub(crate) fn insert<B: Blocker>(&mut self, make: impl FnOnce(BlockerId) -> B) -> BlockerId {
        self.active
            .insert_with_key(|id| Box::new(make(id)) as Box<dyn Blocker>)
    }

    pub(crate) fn readable(&mut self, fd: OwnedFd) -> BlockerId {
        let blocker = self.insert(|_| FdWait);
        self.waits.readable(blocker, fd);
        blocker
    }

    pub(crate) fn acquire(
        &mut self,
        surface: SurfaceId,
        point: TimelinePoint,
    ) -> Option<BlockerId> {
        let blocker = self.insert(|_| FdWait);
        match self.waits.acquire(surface, blocker, point) {
            Ok(()) => Some(blocker),
            Err(error) => {
                tracing::warn!("failed to wait for acquire point: {error}");
                self.active.remove(blocker);
                None
            }
        }
    }

    pub(crate) fn release(&mut self, blocker: BlockerId) -> bool {
        self.waits.retire(blocker);
        self.active.remove(blocker).is_some()
    }

    pub(crate) fn is_active(&self, blocker: BlockerId) -> bool {
        self.active.contains_key(blocker)
    }

    pub(crate) fn release_all(&mut self, blockers: impl IntoIterator<Item = BlockerId>) {
        for blocker in blockers {
            self.release(blocker);
        }
    }
}
