use crate::ids::SurfaceId;
use crate::surfaces::Surfaces;

impl Surfaces {
    pub(crate) fn remove(&mut self, id: SurfaceId) {
        let Some(mut surface) = self.surfaces.remove(id) else {
            return;
        };
        if let Some(parent) = surface.parent {
            self.unlink(parent, id);
        }
        for child in &surface.children {
            if let Some(child) = self.surfaces.get_mut(*child) {
                child.parent = None;
            }
        }
        self.queue.forget(id, &mut self.buffers, &mut self.blockers);
        surface
            .pending
            .discard(&mut self.buffers, &mut self.blockers);
        if let Some(mut cached) = surface.cached.take() {
            cached.discard(&mut self.buffers, &mut self.blockers);
        }
        surface.current.discard(&mut self.buffers);
        self.blockers.waits.forget(id);
        if surface.hints.dmabuf.is_some() {
            self.dmabuf.rebuild(&self.surfaces);
        }
    }
}
