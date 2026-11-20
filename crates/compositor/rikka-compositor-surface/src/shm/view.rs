use rikka_compositor_geometry::{Buffer, Size};
use rikka_compositor_protocols::wl_buffer::WlBuffer;
use rikka_compositor_protocols::wl_shm;
use rikka_compositor_types::Fourcc;
use rikka_wayland_wire::Resource;

use super::pool::Pool;
use super::sigbus;
use crate::buffer::ShmLayout;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AccessError {
    #[error("client truncated the buffer's shared memory")]
    Truncated,
    #[error("buffer memory is mapped read-only")]
    ReadOnly,
}

#[derive(Debug, Clone, Copy)]
pub struct ShmBuffer<'a> {
    pool: &'a Pool,
    resource: &'a WlBuffer,
    layout: ShmLayout,
    size: Size<i32, Buffer>,
}

impl<'a> ShmBuffer<'a> {
    pub(crate) fn new(
        pool: &'a Pool,
        resource: &'a WlBuffer,
        layout: ShmLayout,
        size: Size<i32, Buffer>,
    ) -> Self {
        Self {
            pool,
            resource,
            layout,
            size,
        }
    }

    pub fn size(&self) -> Size<i32, Buffer> {
        self.size
    }

    pub fn stride(&self) -> usize {
        self.layout.stride
    }

    pub fn format(&self) -> Fourcc {
        self.layout.format
    }

    pub fn truncated(&self) -> bool {
        self.pool.truncated()
    }

    pub fn with_pixels<R>(&self, read: impl FnOnce(&[u8]) -> R) -> Result<R, AccessError> {
        self.guarded(|pixels, len| read(unsafe { std::slice::from_raw_parts(pixels, len) }))
    }

    pub fn with_pixels_mut<R>(
        &mut self,
        write: impl FnOnce(&mut [u8]) -> R,
    ) -> Result<R, AccessError> {
        if !self.pool.writable() {
            return Err(AccessError::ReadOnly);
        }
        self.guarded(|pixels, len| {
            write(unsafe { std::slice::from_raw_parts_mut(pixels.cast_mut(), len) })
        })
    }

    fn guarded<R>(&self, access: impl FnOnce(*const u8, usize) -> R) -> Result<R, AccessError> {
        if self.pool.truncated() {
            return Err(AccessError::Truncated);
        }
        let range = self.pool.range();
        let len = self.layout.stride * self.size.h.unsigned_abs() as usize;
        let accessed = sigbus::guarded(range, || {
            access(unsafe { range.0.add(self.layout.offset) }, len)
        });
        accessed.map_err(|_| {
            self.pool.mark_truncated();
            self.resource.object().post_error(
                wl_shm::Error::InvalidFd.code(),
                "client truncated the buffer's shared memory",
            );
            AccessError::Truncated
        })
    }
}
