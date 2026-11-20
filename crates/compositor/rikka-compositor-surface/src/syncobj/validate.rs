use rikka_compositor_protocols::wp_linux_drm_syncobj_surface_v1::Error;

use crate::surface::Surface;
use crate::surfaces::Surfaces;

impl Surfaces {
    pub(crate) fn explicit_sync_valid(&self, surface: &Surface) -> bool {
        let Some(object) = &surface.extensions.syncobj else {
            return true;
        };
        let points = &surface.pending.points;
        let attached = match surface.pending.buffer {
            Some(Some(buffer)) => Some(buffer),
            _ => None,
        };
        let failure = match (attached, &points.acquire, &points.release) {
            (None, None, None) => None,
            (None, _, _) => Some(Error::NoBuffer),
            (Some(buffer), _, _) if self.buffers.dmabuf(buffer).is_none() => {
                Some(Error::UnsupportedBuffer)
            }
            (Some(_), None, _) => Some(Error::NoAcquirePoint),
            (Some(_), _, None) => Some(Error::NoReleasePoint),
            (Some(_), Some(acquire), Some(release))
                if acquire.same_timeline(release) && acquire.point >= release.point =>
            {
                Some(Error::ConflictingPoints)
            }
            (Some(_), Some(_), Some(_)) => None,
        };
        match failure {
            Some(error) => {
                object.post_error(error);
                false
            }
            None => true,
        }
    }
}
