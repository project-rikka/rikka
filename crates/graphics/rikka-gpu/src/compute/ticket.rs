use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use ash::vk;

use crate::compute::context::Shared;
use crate::compute::lane::{Lane, at};
use crate::compute::pool::Use;
use crate::core::buffer::GpuBuffer;
use crate::error::{Error, Result};

#[derive(Clone)]
pub struct Ticket {
    pub shared: Arc<Shared>,
    pub(crate) lane: Lane,
    pub value: u64,
}

impl Ticket {
    pub fn wait(&self) -> Result<()> {
        self.shared.line(self.lane).timeline.wait(self.value)?;
        self.shared.collect();
        Ok(())
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.shared.line(self.lane).timeline.reached(self.value)
    }

    pub fn on_ready(&self, notify: impl FnOnce() + Send + 'static) {
        self.shared
            .completions
            .notify(self.lane, self.value, Box::new(notify));
    }

    pub fn semaphore(&self) -> vk::Semaphore {
        self.shared.line(self.lane).timeline.handle()
    }

    pub fn value(&self) -> u64 {
        self.value
    }
}

impl std::fmt::Debug for Ticket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ticket")
            .field("lane", &self.lane)
            .field("value", &self.value)
            .finish()
    }
}

pub(crate) struct DownloadSlot {
    shared: Arc<Shared>,
    lane: Lane,
    buffer: Option<GpuBuffer>,
    size: usize,
    completion: AtomicU64,
}

impl DownloadSlot {
    pub(crate) fn new(
        shared: &Arc<Shared>,
        lane: Lane,
        buffer: Option<GpuBuffer>,
        size: usize,
    ) -> Arc<DownloadSlot> {
        Arc::new(DownloadSlot {
            shared: Arc::clone(shared),
            lane,
            buffer,
            size,
            completion: AtomicU64::new(0),
        })
    }

    pub(crate) fn handle(&self) -> Option<vk::Buffer> {
        self.buffer.as_ref().map(GpuBuffer::handle)
    }

    pub(crate) fn complete_at(&self, value: u64) {
        self.completion.store(value, Ordering::Release);
    }

    fn completed_value(&self) -> u64 {
        self.completion.load(Ordering::Acquire)
    }
}

impl Drop for DownloadSlot {
    fn drop(&mut self) {
        let Some(buffer) = self.buffer.take() else {
            return;
        };
        let last = at(self.lane, *self.completion.get_mut());
        let mut state = self.shared.state.lock();
        let evicted = state.pool.recycle_buffer(Use::DownloadSlot, buffer, last);
        state.release_eviction(evicted);
    }
}

#[must_use = "a download is only read by waiting on it"]
pub struct Download {
    pub(crate) download_slot: Arc<DownloadSlot>,
}

impl Download {
    pub fn wait(self) -> Result<Vec<u8>> {
        let download_slot = &self.download_slot;
        let ready = match download_slot.completed_value() {
            0 => {
                return Err(Error::Invalid(
                    "download read before its batch was submitted".into(),
                ));
            }
            ready => ready,
        };
        let shared = Arc::clone(&download_slot.shared);
        shared.line(download_slot.lane).timeline.wait(ready)?;
        let bytes = download_slot
            .buffer
            .as_ref()
            .and_then(GpuBuffer::mapped_slice)
            .map(|mapped| mapped[..download_slot.size].to_vec())
            .ok_or_else(|| Error::Unsupported("download memory is not host-visible".into()))?;
        drop(self);
        shared.collect();
        Ok(bytes)
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        let ready = self.download_slot.completed_value();
        ready != 0
            && self
                .download_slot
                .shared
                .line(self.download_slot.lane)
                .timeline
                .reached(ready)
    }
}

impl std::fmt::Debug for Download {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Download")
            .field("size", &self.download_slot.size)
            .finish_non_exhaustive()
    }
}
