use crate::core::image::GpuImage;
use crate::drm::fence::ExportSemaphore;
use crate::identity::BufferId;
use crate::reclaim::{Deferred, ReclaimRequest, Reclaimer};

pub struct DmabufTarget {
    pub image: Deferred<GpuImage>,
    pub buffer: BufferId,
    pub readable: bool,
    pub fence: Option<Deferred<ExportSemaphore>>,
    pub reclaimer: Reclaimer,
}

impl DmabufTarget {
    pub fn width(&self) -> u32 {
        self.image.extent().width
    }

    pub fn height(&self) -> u32 {
        self.image.extent().height
    }
}

impl Drop for DmabufTarget {
    fn drop(&mut self) {
        self.reclaimer.enqueue(ReclaimRequest::Buffer(self.buffer));
    }
}

impl std::fmt::Debug for DmabufTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DmabufTarget")
            .field("width", &self.width())
            .field("height", &self.height())
            .finish_non_exhaustive()
    }
}
