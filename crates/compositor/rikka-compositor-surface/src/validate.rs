use rikka_compositor_protocols::{wl_surface, wp_viewport};

use crate::committed::{BufferChange, PreCommit};
use crate::ids::SurfaceId;
use crate::surfaces::Surfaces;

impl Surfaces {
    pub(crate) fn validate(&self, id: SurfaceId) -> Option<PreCommit> {
        let surface = self.surfaces.get(id)?;
        let pending = &surface.pending;
        if !pending.release_callbacks.is_empty() && !matches!(pending.buffer, Some(Some(_))) {
            surface.resource.post_error(wl_surface::Error::NoBuffer);
            return None;
        }
        let latest = surface.latest.after(pending, &self.buffers);
        let geometry = latest.geometry;
        if !geometry.divisible() {
            surface.resource.post_error(wl_surface::Error::InvalidSize);
            return None;
        }
        if let Some(viewport) = &surface.extensions.viewport {
            if !geometry.source_integral() {
                viewport.post_error(wp_viewport::Error::BadSize);
                return None;
            }
            if !geometry.source_fits() {
                viewport.post_error(wp_viewport::Error::OutOfBuffer);
                return None;
            }
        }
        if !self.explicit_sync_valid(surface) || !self.representation_valid(surface, &latest) {
            return None;
        }
        let buffer = match pending.buffer {
            None => BufferChange::Unchanged,
            Some(Some(buffer)) if self.buffers.alive(buffer) => BufferChange::Attached,
            Some(_) => BufferChange::Removed,
        };
        Some(PreCommit {
            surface: id,
            buffer,
            size: geometry.size(),
            synced: self.synced(id),
        })
    }
}
