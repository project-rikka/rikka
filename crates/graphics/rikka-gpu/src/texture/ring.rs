use std::collections::VecDeque;
use std::sync::Arc;

use ash::vk;
use rikka_sync::Mutex;

use crate::core::buffer::GpuBuffer;
use crate::core::device::RenderDevice;
use crate::core::frame::FrameTimeline;
use crate::core::memory::Placement;
use crate::error::Result;

const SMALLEST_BLOCK: vk::DeviceSize = 4 * 1024 * 1024;

const ALIGN: vk::DeviceSize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Retire {
    Frame(u64),
    Transfer(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SpanState {
    Writing,
    Unused,
    After(Retire),
}

struct Record {
    start: vk::DeviceSize,
    end: vk::DeviceSize,
    state: SpanState,
}

struct Block {
    id: u64,
    buffer: GpuBuffer,
    base: usize,
    records: VecDeque<Record>,
    front: u64,
}

impl Block {
    fn place(&self, len: vk::DeviceSize) -> Option<vk::DeviceSize> {
        let size = self.buffer.size();
        let (Some(oldest), Some(newest)) = (self.records.front(), self.records.back()) else {
            return (len <= size).then_some(0);
        };
        let head = newest.end.next_multiple_of(ALIGN);
        match head > oldest.start {
            true if head + len <= size => Some(head),
            true if len <= oldest.start => Some(0),
            false if head + len <= oldest.start => Some(head),
            _ => None,
        }
    }

    fn reclaim(&mut self, reached: impl Fn(Retire) -> bool) {
        while let Some(oldest) = self.records.front() {
            let done = match oldest.state {
                SpanState::Writing => false,
                SpanState::Unused => true,
                SpanState::After(retire) => reached(retire),
            };
            if !done {
                break;
            }
            self.records.pop_front();
            self.front += 1;
        }
    }
}

#[derive(Default)]
struct Blocks {
    blocks: Vec<Block>,
    minted: u64,
}

pub struct UploadRing {
    device: Arc<RenderDevice>,
    frames: Arc<FrameTimeline>,
    transfers: Option<Arc<FrameTimeline>>,
    blocks: Mutex<Blocks>,
}

pub struct RingSpan {
    ring: Arc<UploadRing>,
    block: u64,
    sequence: u64,
    buffer: vk::Buffer,
    offset: vk::DeviceSize,
    len: usize,
    at: usize,
    retire: Option<Retire>,
}

impl UploadRing {
    pub fn new(
        device: Arc<RenderDevice>,
        frames: Arc<FrameTimeline>,
        transfers: Option<Arc<FrameTimeline>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            device,
            frames,
            transfers,
            blocks: Mutex::default(),
        })
    }

    pub(crate) fn reserve(self: &Arc<Self>, len: usize) -> Result<RingSpan> {
        let size = (len as vk::DeviceSize).max(1);
        let reached = self.reached();
        let mut pool = self.blocks.lock();
        for block in &mut pool.blocks {
            block.reclaim(&reached);
        }
        let found = pool
            .blocks
            .iter()
            .enumerate()
            .find_map(|(index, block)| block.place(size).map(|start| (index, start)));
        let (index, start) = match found {
            Some(found) => found,
            None => {
                let capacity = (size * 4).next_power_of_two().max(SMALLEST_BLOCK);
                let shared_by = self
                    .device
                    .transfer_family()
                    .map(|transfer| [self.device.queue_family(), transfer]);
                let mut buffer = GpuBuffer::create(
                    self.device.clone(),
                    "rikka.upload_ring",
                    capacity,
                    vk::BufferUsageFlags::TRANSFER_SRC,
                    Placement::Staging,
                    shared_by,
                )?;
                let base = buffer
                    .mapped_slice_mut()
                    .expect("staging memory is host-visible")
                    .as_mut_ptr() as usize;
                pool.minted += 1;
                let id = pool.minted;
                pool.blocks.push(Block {
                    id,
                    buffer,
                    base,
                    records: VecDeque::new(),
                    front: 0,
                });
                (pool.blocks.len() - 1, 0)
            }
        };
        let block = &mut pool.blocks[index];
        let sequence = block.front + block.records.len() as u64;
        block.records.push_back(Record {
            start,
            end: start + size,
            state: SpanState::Writing,
        });
        Ok(RingSpan {
            ring: Arc::clone(self),
            block: block.id,
            sequence,
            buffer: block.buffer.handle(),
            offset: start,
            len,
            at: block.base + start as usize,
            retire: None,
        })
    }

    pub(crate) fn trim(&self) {
        let reached = self.reached();
        let mut pool = self.blocks.lock();
        for block in &mut pool.blocks {
            block.reclaim(&reached);
        }
        pool.blocks.retain(|block| !block.records.is_empty());
    }

    fn reached(&self) -> impl Fn(Retire) -> bool {
        let frames = self.frames.completed();
        let transfers = self
            .transfers
            .as_ref()
            .map_or(0, |timeline| timeline.completed());
        move |retire| match retire {
            Retire::Frame(value) => value <= frames,
            Retire::Transfer(value) => value <= transfers,
        }
    }

    fn release(&self, block: u64, sequence: u64, retire: Option<Retire>) {
        let mut pool = self.blocks.lock();
        let Some(block) = pool
            .blocks
            .iter_mut()
            .find(|candidate| candidate.id == block)
        else {
            return;
        };
        let Some(record) = sequence
            .checked_sub(block.front)
            .and_then(|index| block.records.get_mut(index as usize))
        else {
            return;
        };
        record.state = retire.map_or(SpanState::Unused, SpanState::After);
    }
}

impl RingSpan {
    pub(crate) fn bytes_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.at as *mut u8, self.len) }
    }

    pub(crate) fn buffer(&self) -> vk::Buffer {
        self.buffer
    }

    pub(crate) fn offset(&self) -> vk::DeviceSize {
        self.offset
    }

    pub(crate) fn retire_after(&mut self, retire: Retire) {
        self.retire = Some(retire);
    }
}

impl Drop for RingSpan {
    fn drop(&mut self) {
        self.ring.release(self.block, self.sequence, self.retire);
    }
}

impl std::fmt::Debug for UploadRing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UploadRing").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for RingSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RingSpan").finish_non_exhaustive()
    }
}
