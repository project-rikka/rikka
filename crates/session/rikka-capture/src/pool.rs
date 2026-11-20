use rikka_dmabuf_convert::Imported;
use rikka_gpu::{DmabufSpec, Gpu};

use crate::buffer::CaptureBuffer;
use crate::constraints::Constraints;
use crate::error::{Error, Result};
use crate::frames::Frames;
use crate::screen::Screen;

struct Slot {
    buffer: CaptureBuffer,
    imported: Option<Imported>,
}

pub struct BufferPool {
    slots: Vec<Slot>,
    generation: u64,
    pending: Option<usize>,
    latest: Option<usize>,
}

impl BufferPool {
    pub fn dmabuf(
        screen: &mut Screen,
        gpu: &Gpu,
        constraints: &Constraints,
        count: usize,
    ) -> Result<BufferPool> {
        let offered = constraints.dmabuf.as_ref().ok_or(Error::NoSharedBuffer)?;
        let (format, modifiers) = offered.sampled_by(gpu).ok_or(Error::NoSharedBuffer)?;
        let buffers = (0..count)
            .map(|_| {
                screen.dmabuf_buffer(
                    offered.device,
                    format,
                    &modifiers,
                    constraints.width,
                    constraints.height,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(BufferPool::of(buffers, constraints.generation))
    }

    pub fn shm(screen: &mut Screen, constraints: &Constraints, count: usize) -> Result<BufferPool> {
        let format = constraints.best_shm().ok_or(Error::NoFormat)?;
        let buffers = (0..count)
            .map(|_| screen.shm_buffer(format, constraints.width, constraints.height))
            .collect::<Result<Vec<_>>>()?;
        Ok(BufferPool::of(buffers, constraints.generation))
    }

    fn of(buffers: Vec<CaptureBuffer>, generation: u64) -> BufferPool {
        BufferPool {
            slots: buffers
                .into_iter()
                .map(|buffer| Slot {
                    buffer,
                    imported: None,
                })
                .collect(),
            generation,
            pending: None,
            latest: None,
        }
    }

    pub fn bind<E>(
        &mut self,
        mut import: impl FnMut(&DmabufSpec<'_>) -> std::result::Result<Imported, E>,
    ) -> std::result::Result<(), E> {
        for slot in &mut self.slots {
            slot.imported = slot.buffer.with_dmabuf(&mut import).transpose()?;
        }
        Ok(())
    }

    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    #[must_use]
    pub fn latest(&self) -> Option<usize> {
        self.latest
    }

    #[must_use]
    pub fn buffer(&self, index: usize) -> &CaptureBuffer {
        &self.slots[index].buffer
    }

    #[must_use]
    pub fn imported(&self, index: usize) -> Option<&Imported> {
        self.slots[index].imported.as_ref()
    }

    #[must_use]
    pub fn is_idle(&self, index: usize) -> bool {
        self.slots[index]
            .imported
            .as_ref()
            .is_none_or(Imported::is_idle)
    }

    pub fn capture(&mut self, frames: &mut Frames) -> Result<bool> {
        if self.pending.is_some() || frames.is_capturing() {
            return Ok(false);
        }
        let free =
            (0..self.slots.len()).find(|index| Some(*index) != self.latest && self.is_idle(*index));
        let Some(index) = free else {
            return Ok(false);
        };
        frames.capture(&self.slots[index].buffer)?;
        self.pending = Some(index);
        Ok(true)
    }

    pub(crate) fn landed(&mut self) -> Option<usize> {
        let index = self.pending.take()?;
        self.latest = Some(index);
        Some(index)
    }

    pub(crate) fn lost(&mut self) {
        self.pending = None;
    }
}

impl std::fmt::Debug for BufferPool {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BufferPool")
            .field("buffers", &self.slots.len())
            .field("generation", &self.generation)
            .field("pending", &self.pending)
            .field("latest", &self.latest)
            .finish()
    }
}
