use rikka_compositor_shell::LockSurfaceId;
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_types::OutputId;

use crate::desktop::Desktop;
use crate::error::{Error, Result};
use crate::lock::Guard;

impl<P> Desktop<P> {
    pub fn lock(&mut self) {
        self.lock.locked = true;
    }

    pub fn unlock(&mut self) {
        self.lock.locked = false;
        self.lock.surfaces.clear();
    }

    pub fn locked(&self) -> bool {
        self.lock.locked
    }

    pub fn add_lock_surface(
        &mut self,
        id: LockSurfaceId,
        output: OutputId,
        surface: SurfaceId,
    ) -> Result<()> {
        self.output_record(output)
            .ok_or(Error::UnknownOutput(output))?;
        if self.lock.surfaces.contains_key(id) {
            return Err(Error::DuplicateLockSurface(id));
        }
        self.lock.surfaces.insert(id, Guard { surface, output });
        Ok(())
    }

    pub fn remove_lock_surface(&mut self, id: LockSurfaceId) -> bool {
        self.lock.surfaces.remove(id).is_some()
    }

    pub fn lock_surface_on(&self, output: OutputId) -> Option<(LockSurfaceId, SurfaceId)> {
        self.lock
            .surfaces
            .iter()
            .find(|(_, guard)| guard.output == output)
            .map(|(id, guard)| (id, guard.surface))
    }
}
