use rikka_compositor_protocols::ext_session_lock_v1::ExtSessionLockV1;
use slotmap::SlotMap;

use super::surface::LockSurface;
use crate::ids::{LockId, LockSurfaceId};
use crate::shell::Shell;

#[derive(Debug)]
pub(crate) struct LockObject {
    pub(crate) resource: ExtSessionLockV1,
    pub(crate) locked_sent: bool,
    pub(crate) finished: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Locks {
    pub(crate) objects: SlotMap<LockId, LockObject>,
    pub(crate) surfaces: SlotMap<LockSurfaceId, LockSurface>,
    pub(crate) holder: Option<LockId>,
    pub(crate) locked: bool,
}

impl Shell {
    pub fn session_locked(&self) -> bool {
        self.locks.locked
    }

    pub fn session_lock(&self) -> Option<LockId> {
        self.locks.holder
    }

    pub fn lock_session(&mut self, lock: LockId) -> bool {
        let locks = &mut self.locks;
        if locks.holder != Some(lock) {
            return false;
        }
        let Some(object) = locks.objects.get_mut(lock) else {
            return false;
        };
        if !object.locked_sent {
            object.locked_sent = true;
            object.resource.locked();
        }
        locks.locked = true;
        true
    }

    pub fn finish_lock(&mut self, lock: LockId) {
        let locks = &mut self.locks;
        let Some(object) = locks.objects.get_mut(lock) else {
            return;
        };
        if !object.finished {
            object.finished = true;
            object.resource.finished();
        }
        if locks.holder == Some(lock) {
            locks.holder = None;
            locks.locked = false;
        }
    }

    pub fn set_session_locked(&mut self) {
        self.locks.locked = true;
    }

    pub fn unlock_session(&mut self) {
        if let Some(holder) = self.locks.holder.take() {
            self.finish_lock(holder);
        }
        self.locks.locked = false;
    }

    pub fn lock_surface(&self, surface: LockSurfaceId) -> Option<&LockSurface> {
        self.locks.surfaces.get(surface)
    }

    pub fn lock_surfaces(&self) -> impl Iterator<Item = (LockSurfaceId, &LockSurface)> {
        self.locks.surfaces.iter()
    }
}
