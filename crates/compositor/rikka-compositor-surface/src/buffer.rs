use rikka_compositor_geometry::{Buffer, Size};
use rikka_compositor_protocols::wl_buffer::WlBuffer;
use rikka_compositor_protocols::wl_callback::WlCallback;
use rikka_compositor_types::Fourcc;
use slotmap::SlotMap;

use crate::content::{BufferContent, Rgba};
use crate::dmabuf::Dmabuf;
use crate::ids::{BufferId, PoolId, UseId};
use crate::opacity::without_alpha;
use crate::shm::{Pools, ShmBuffer};
use crate::syncobj::TimelinePoint;

const GONE_HELD: usize = 64;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShmLayout {
    pub(crate) pool: PoolId,
    pub(crate) offset: usize,
    pub(crate) stride: usize,
    pub(crate) format: Fourcc,
}

#[derive(Debug)]
pub(crate) enum Storage {
    Shm(ShmLayout),
    SinglePixel(Rgba),
    Dmabuf(Box<Dmabuf>),
}

#[derive(Debug)]
struct Entry {
    resource: WlBuffer,
    storage: Storage,
    size: Size<i32, Buffer>,
    alive: bool,
    uses: u32,
}

#[derive(Debug)]
struct Use {
    buffer: BufferId,
    holds: u32,
    release_callbacks: Vec<WlCallback>,
    release_point: Option<TimelinePoint>,
}

#[derive(Debug, Default)]
pub(crate) struct Buffers {
    entries: SlotMap<BufferId, Entry>,
    uses: SlotMap<UseId, Use>,
    pub(crate) pools: Pools,
    gone: Vec<Dmabuf>,
}

impl Buffers {
    pub(crate) fn create(
        &mut self,
        resource: WlBuffer,
        storage: Storage,
        size: Size<i32, Buffer>,
    ) -> BufferId {
        if let Storage::Shm(layout) = &storage {
            self.pools.retain(layout.pool);
        }
        self.entries.insert(Entry {
            resource,
            storage,
            size,
            alive: true,
            uses: 0,
        })
    }

    pub(crate) fn alive(&self, buffer: BufferId) -> bool {
        self.entries.get(buffer).is_some_and(|entry| entry.alive)
    }

    pub(crate) fn size(&self, buffer: BufferId) -> Option<Size<i32, Buffer>> {
        self.entries
            .get(buffer)
            .filter(|entry| entry.alive)
            .map(|entry| entry.size)
    }

    pub(crate) fn used(&self, used: UseId) -> Option<(BufferId, Size<i32, Buffer>)> {
        let buffer = self.uses.get(used)?.buffer;
        Some((buffer, self.entries.get(buffer)?.size))
    }

    pub(crate) fn begin_use(
        &mut self,
        buffer: BufferId,
        release_callbacks: Vec<WlCallback>,
    ) -> Option<UseId> {
        let entry = self.entries.get_mut(buffer).filter(|entry| entry.alive)?;
        entry.uses += 1;
        Some(self.uses.insert(Use {
            buffer,
            holds: 1,
            release_callbacks,
            release_point: None,
        }))
    }

    pub(crate) fn set_release_point(&mut self, used: UseId, point: TimelinePoint) {
        if let Some(record) = self.uses.get_mut(used) {
            record.release_point = Some(point);
        }
    }

    pub(crate) fn dmabuf(&self, buffer: BufferId) -> Option<&Dmabuf> {
        match &self.entries.get(buffer)?.storage {
            Storage::Dmabuf(dmabuf) => Some(dmabuf),
            Storage::Shm(_) | Storage::SinglePixel(_) => None,
        }
    }

    pub(crate) fn format(&self, buffer: BufferId) -> Option<Fourcc> {
        Some(match &self.entries.get(buffer)?.storage {
            Storage::Shm(layout) => layout.format,
            Storage::SinglePixel(_) => Fourcc::ARGB8888,
            Storage::Dmabuf(dmabuf) => dmabuf.format.fourcc,
        })
    }

    pub(crate) fn opaque(&self, buffer: BufferId) -> bool {
        match self.entries.get(buffer).map(|entry| &entry.storage) {
            Some(Storage::Shm(layout)) => without_alpha(layout.format),
            Some(Storage::Dmabuf(dmabuf)) => without_alpha(dmabuf.format.fourcc),
            Some(Storage::SinglePixel(rgba)) => rgba.a == u32::MAX,
            None => false,
        }
    }

    pub(crate) fn used_buffer(&self, used: UseId) -> Option<BufferId> {
        Some(self.uses.get(used)?.buffer)
    }

    pub(crate) fn hold(&mut self, used: UseId) -> bool {
        match self.uses.get_mut(used) {
            Some(record) => {
                record.holds += 1;
                true
            }
            None => false,
        }
    }

    pub(crate) fn drop_hold(&mut self, used: UseId) {
        let Some(record) = self.uses.get_mut(used) else {
            return;
        };
        record.holds -= 1;
        if record.holds > 0 {
            return;
        }
        let Some(record) = self.uses.remove(used) else {
            return;
        };
        for callback in record.release_callbacks {
            callback.done(0);
        }
        if let Some(point) = record.release_point {
            point.signal();
        }
        let Some(entry) = self.entries.get_mut(record.buffer) else {
            return;
        };
        entry.uses -= 1;
        if entry.uses > 0 {
            return;
        }
        if entry.alive {
            entry.resource.release();
        } else {
            self.forget(record.buffer);
        }
    }

    pub(crate) fn destroyed(&mut self, buffer: BufferId) {
        let Some(entry) = self.entries.get_mut(buffer) else {
            return;
        };
        entry.alive = false;
        if entry.uses == 0 {
            self.forget(buffer);
        }
    }

    pub(crate) fn content(&self, buffer: BufferId) -> Option<BufferContent<'_>> {
        let entry = self.entries.get(buffer)?;
        Some(match &entry.storage {
            Storage::Shm(layout) => BufferContent::Shm(ShmBuffer::new(
                self.pools.get(layout.pool)?,
                &entry.resource,
                *layout,
                entry.size,
            )),
            Storage::SinglePixel(rgba) => BufferContent::SinglePixel(*rgba),
            Storage::Dmabuf(dmabuf) => BufferContent::Dmabuf(dmabuf),
        })
    }

    fn forget(&mut self, buffer: BufferId) {
        let Some(entry) = self.entries.remove(buffer) else {
            return;
        };
        match entry.storage {
            Storage::Shm(layout) => self.pools.release(layout.pool),
            Storage::Dmabuf(dmabuf) => {
                if self.gone.len() == GONE_HELD {
                    self.gone.remove(0);
                }
                self.gone.push(*dmabuf);
            }
            Storage::SinglePixel(_) => {}
        }
    }

    pub(crate) fn take_gone(&mut self) -> std::vec::Drain<'_, Dmabuf> {
        self.gone.drain(..)
    }
}
