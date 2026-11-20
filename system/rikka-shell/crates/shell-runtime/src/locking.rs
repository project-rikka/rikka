use rikka_wayland_client::Proxy;
use rikka_wayland_client::ext_session_lock_manager_v1::ExtSessionLockManagerV1;
use rikka_wayland_client::ext_session_lock_v1::ExtSessionLockV1;
use rikka_wayland_client::wl_output::WlOutput;
use rikka_wayland_client::wl_surface::WlSurface;

use super::role::LockSurface;

#[derive(Debug, Default)]
enum LockStatus {
    #[default]
    Free,
    Asked(ExtSessionLockV1),
    Locked(ExtSessionLockV1),
    Withdrawn(ExtSessionLockV1),
}

#[derive(Debug)]
pub(super) struct Locker {
    manager: Option<ExtSessionLockManagerV1>,
    status: LockStatus,
}

impl Locker {
    pub(crate) fn new(manager: Option<ExtSessionLockManagerV1>) -> Locker {
        Locker {
            manager,
            status: LockStatus::Free,
        }
    }

    pub(super) fn can_lock(&self) -> bool {
        self.manager.is_some()
    }

    pub(crate) fn guard(&mut self, surface: WlSurface, output: &WlOutput) -> Option<LockSurface> {
        let manager = self.manager.as_ref()?;
        self.status = match std::mem::take(&mut self.status) {
            LockStatus::Free => LockStatus::Asked(manager.lock()),
            LockStatus::Withdrawn(lock) => LockStatus::Asked(lock),
            status => status,
        };
        let (LockStatus::Asked(lock) | LockStatus::Locked(lock)) = &self.status else {
            return None;
        };
        let role = lock
            .get_lock_surface(&surface, output)
            .with_key(surface.object_id());
        Some(LockSurface::new(role, surface))
    }

    pub(super) fn is_free(&self) -> bool {
        matches!(self.status, LockStatus::Free)
    }

    pub(super) fn handle_locked(&mut self) -> bool {
        self.status = match std::mem::take(&mut self.status) {
            LockStatus::Asked(lock) | LockStatus::Locked(lock) => LockStatus::Locked(lock),
            LockStatus::Withdrawn(lock) => {
                lock.unlock_and_destroy();
                LockStatus::Free
            }
            LockStatus::Free => LockStatus::Free,
        };
        matches!(self.status, LockStatus::Locked(_))
    }

    pub(super) fn handle_finished(&mut self) {
        match std::mem::take(&mut self.status) {
            LockStatus::Locked(lock) => lock.unlock_and_destroy(),
            LockStatus::Asked(lock) | LockStatus::Withdrawn(lock) => lock.destroy(),
            LockStatus::Free => {}
        }
    }

    pub(crate) fn release(&mut self) {
        self.status = match std::mem::take(&mut self.status) {
            LockStatus::Asked(lock) | LockStatus::Withdrawn(lock) => LockStatus::Withdrawn(lock),
            LockStatus::Locked(lock) => {
                lock.unlock_and_destroy();
                LockStatus::Free
            }
            LockStatus::Free => LockStatus::Free,
        };
    }
}
