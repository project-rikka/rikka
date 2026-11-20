use rikka_compositor_protocols::wl_buffer::WlBuffer;

use crate::content::BufferContent;
use crate::dmabuf::Dmabuf;
use crate::ids::{BufferId, SurfaceId, UseId};
use crate::surfaces::Surfaces;

#[must_use = "a held buffer is only released through Surfaces::release"]
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct BufferHold {
    used: UseId,
}

impl Surfaces {
    pub fn content(&self, surface: SurfaceId) -> Option<BufferContent<'_>> {
        let attached = self.surfaces.get(surface)?.current.buffer?;
        self.buffers.content(attached.buffer)
    }

    pub fn buffer_content(&self, buffer: &WlBuffer) -> Option<BufferContent<'_>> {
        self.buffers.content(buffer.key::<BufferId>()?)
    }
    pub fn take_destroyed_dmabufs(&mut self) -> impl Iterator<Item = Dmabuf> + '_ {
        self.buffers.take_gone()
    }

    pub fn hold(&mut self, surface: SurfaceId) -> Option<BufferHold> {
        let used = self.surfaces.get(surface)?.current.buffer?.use_id?;
        self.buffers.hold(used).then_some(BufferHold { used })
    }

    pub fn release(&mut self, hold: BufferHold) {
        self.buffers.drop_hold(hold.used);
    }

    pub fn uploaded(&mut self, surface: SurfaceId) {
        let Some(attached) = self
            .surfaces
            .get_mut(surface)
            .and_then(|surface| surface.current.buffer.as_mut())
        else {
            return;
        };
        if let Some(used) = attached.use_id.take() {
            self.buffers.drop_hold(used);
        }
    }
}
