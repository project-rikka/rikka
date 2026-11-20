use rikka_compositor_geometry::Point;

use crate::host::SurfaceHost;
use crate::ids::SurfaceId;
use crate::role::Role;
use crate::state::Placement;
use crate::surfaces::Surfaces;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Subsurface;

impl<S: SurfaceHost> Role<S> for Subsurface {
    const NAME: &'static str = "wl_subsurface";
}

impl Surfaces {
    pub(crate) fn link(&mut self, child: SurfaceId, parent: SurfaceId) {
        if let Some(surface) = self.surfaces.get_mut(child) {
            surface.parent = Some(parent);
            surface.sync = true;
            surface.position = Point::zero();
        }
        if let Some(surface) = self.surfaces.get_mut(parent) {
            surface.children.push(child);
            surface
                .pending
                .update
                .placements
                .push(Placement::Add(child));
        }
    }

    pub(crate) fn detach(&mut self, child: SurfaceId) {
        if self.end_role::<Subsurface>(child).is_none() {
            return;
        }
        let Some(surface) = self.surfaces.get_mut(child) else {
            return;
        };
        let parent = surface.parent.take();
        surface.sync = false;
        let cached = surface.cached.take();
        if let Some(parent) = parent {
            self.unlink(parent, child);
        }
        if let Some(mut cached) = cached {
            cached.discard(&mut self.buffers, &mut self.blockers);
            self.recycle(cached);
        }
    }

    pub(crate) fn unlink(&mut self, parent: SurfaceId, child: SurfaceId) {
        if let Some(parent) = self.surfaces.get_mut(parent) {
            parent.children.retain(|entry| *entry != child);
            parent.stack.retain(|entry| *entry != child);
        }
        self.invalidate_extents(parent);
    }
}
