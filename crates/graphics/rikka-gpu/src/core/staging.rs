use std::ops::Range;
use std::sync::Arc;

use ash::vk;

use crate::core::buffer::GpuBuffer;
use crate::core::device::RenderDevice;
use crate::core::provision::Provision;
use crate::error::Result;

const MIN_CHUNK_SIZE: vk::DeviceSize = 256 * 1024;

const MAX_CHUNK_SIZE: vk::DeviceSize = 1024 * 1024;

const MAX_CHUNKS: usize = 8;

const KEPT_SPARE_BYTES: vk::DeviceSize = 16 * 1024 * 1024;

const ALIGN: vk::DeviceSize = 16;

#[derive(Clone, Copy, Debug)]
pub struct StagingSlice {
    pub buffer: vk::Buffer,
    pub offset: vk::DeviceSize,
}

#[derive(Clone, Debug)]
pub struct StagedCopies {
    pub buffer: vk::Buffer,
    pub copies: Range<usize>,
}

struct Chunk {
    buffer: GpuBuffer,
    cursor: vk::DeviceSize,
    used_by: u64,
}

struct Dedicated {
    buffer: GpuBuffer,
    used_by: u64,
}

pub struct StagingBelt {
    device: Arc<RenderDevice>,
    chunks: Vec<Chunk>,
    dedicated: Vec<Dedicated>,
    unneeded: Vec<GpuBuffer>,
    coming: Vec<(vk::DeviceSize, Provision<GpuBuffer>)>,
    frame: u64,
    completed: u64,
}

impl StagingBelt {
    pub(crate) fn new(device: Arc<RenderDevice>) -> Self {
        Self {
            device,
            chunks: Vec::new(),
            dedicated: Vec::new(),
            unneeded: Vec::new(),
            coming: Vec::new(),
            frame: 0,
            completed: 0,
        }
    }

    pub(crate) fn open(&mut self, frame: u64, completed: u64) {
        self.frame = frame;
        self.completed = completed;
        for chunk in &mut self.chunks {
            if chunk.used_by <= completed {
                chunk.cursor = 0;
            }
        }
        if self.chunks.len() > MAX_CHUNKS {
            self.chunks.sort_by_key(|chunk| {
                (
                    chunk.used_by <= completed,
                    std::cmp::Reverse(chunk.buffer.size()),
                )
            });
            while self.chunks.len() > MAX_CHUNKS
                && let Some(chunk) = self.chunks.pop_if(|chunk| chunk.used_by <= completed)
            {
                self.unneeded.push(chunk.buffer);
            }
        }
        self.take_provisioned();
        self.trim_spare_dedicated();
    }

    fn take_provisioned(&mut self) {
        let dedicated = &mut self.dedicated;
        self.coming.retain(|(_, coming)| match coming.take() {
            None => true,
            Some(Ok(buffer)) => {
                dedicated.push(Dedicated { buffer, used_by: 0 });
                false
            }
            Some(Err(error)) => {
                tracing::debug!("failed to preallocate staging buffer: {error}");
                false
            }
        });
    }

    fn provide_spare(&mut self, taken: usize) {
        let size = self.dedicated[taken].buffer.size();
        let completed = self.completed;
        let spare = self.dedicated.iter().enumerate().any(|(index, dedicated)| {
            index != taken && dedicated.used_by <= completed && dedicated.buffer.size() == size
        });
        if spare || self.coming.iter().any(|(coming, _)| *coming == size) {
            return;
        }
        let provision = Provision::start(&self.device, move |device| {
            GpuBuffer::staging(device, "rikka.staging", size)
        });
        self.coming.push((size, provision));
    }

    pub(crate) fn take_unneeded(&mut self) -> impl Iterator<Item = GpuBuffer> + '_ {
        self.unneeded.drain(..)
    }

    pub fn stage(&mut self, data: &[u8]) -> Result<StagingSlice> {
        let (slice, target) = self.reserve(data.len() as vk::DeviceSize)?;
        target.copy_from_slice(data);
        Ok(slice)
    }

    pub fn stage_gathered(
        &mut self,
        data: &[u8],
        copies: &mut Vec<vk::BufferCopy>,
        staged: &mut Vec<StagedCopies>,
    ) -> Result<()> {
        staged.clear();
        split_longer_than(copies, MAX_CHUNK_SIZE);
        let mut first = 0;
        while first < copies.len() {
            let mut total = copies[first].size;
            let mut end = first + 1;
            while end < copies.len() && total + copies[end].size <= MAX_CHUNK_SIZE {
                total += copies[end].size;
                end += 1;
            }
            let (slice, target) = self.reserve(total)?;
            let mut written = 0;
            for copy in &mut copies[first..end] {
                let from = copy.src_offset as usize;
                let size = copy.size as usize;
                target[written..written + size].copy_from_slice(&data[from..from + size]);
                copy.src_offset = slice.offset + written as vk::DeviceSize;
                written += size;
            }
            staged.push(StagedCopies {
                buffer: slice.buffer,
                copies: first..end,
            });
            first = end;
        }
        Ok(())
    }

    fn reserve(&mut self, size: vk::DeviceSize) -> Result<(StagingSlice, &mut [u8])> {
        if size > MAX_CHUNK_SIZE {
            return self.reserve_dedicated(size);
        }
        let index = self.chunk_with_room(size)?;
        let chunk = &mut self.chunks[index];
        let offset = chunk.cursor.next_multiple_of(ALIGN);
        chunk.cursor = offset + size;
        chunk.used_by = self.frame;
        let slice = StagingSlice {
            buffer: chunk.buffer.handle(),
            offset,
        };
        let target = &mut chunk
            .buffer
            .mapped_slice_mut()
            .expect("staging memory is host-visible")[offset as usize..(offset + size) as usize];
        Ok((slice, target))
    }

    fn reserve_dedicated(&mut self, size: vk::DeviceSize) -> Result<(StagingSlice, &mut [u8])> {
        let completed = self.completed;
        let fitting = self
            .dedicated
            .iter()
            .enumerate()
            .filter(|(_, dedicated)| {
                dedicated.used_by <= completed
                    && dedicated.buffer.size() >= size
                    && dedicated.buffer.size() <= size * 2
            })
            .min_by_key(|(_, dedicated)| dedicated.buffer.size())
            .map(|(index, _)| index);
        let index = match fitting {
            Some(index) => index,
            None => {
                let buffer = GpuBuffer::staging(
                    self.device.clone(),
                    "rikka.staging",
                    size.next_power_of_two().max(MIN_CHUNK_SIZE),
                )?;
                self.dedicated.push(Dedicated { buffer, used_by: 0 });
                self.dedicated.len() - 1
            }
        };
        self.dedicated[index].used_by = self.frame;
        self.provide_spare(index);
        let dedicated = &mut self.dedicated[index];
        let slice = StagingSlice {
            buffer: dedicated.buffer.handle(),
            offset: 0,
        };
        let target = &mut dedicated
            .buffer
            .mapped_slice_mut()
            .expect("staging memory is host-visible")[..size as usize];
        Ok((slice, target))
    }

    fn chunk_with_room(&mut self, size: vk::DeviceSize) -> Result<usize> {
        let fits =
            |chunk: &Chunk| chunk.cursor.next_multiple_of(ALIGN) + size <= chunk.buffer.size();
        if let Some(index) = self.chunks.iter().position(fits) {
            return Ok(index);
        }
        let largest = self.chunks.iter().map(|chunk| chunk.buffer.size()).max();
        let buffer = GpuBuffer::staging(
            self.device.clone(),
            "rikka.staging",
            chunk_size(size, largest),
        )?;
        self.chunks.push(Chunk {
            buffer,
            cursor: 0,
            used_by: 0,
        });
        Ok(self.chunks.len() - 1)
    }

    fn trim_spare_dedicated(&mut self) {
        let completed = self.completed;
        self.dedicated
            .sort_by_key(|dedicated| std::cmp::Reverse(dedicated.used_by));
        let mut spare_bytes: vk::DeviceSize = 0;
        let mut at = 0;
        while at < self.dedicated.len() {
            let dedicated = &self.dedicated[at];
            let spare = dedicated.used_by <= completed;
            if spare && spare_bytes > 0 && spare_bytes + dedicated.buffer.size() > KEPT_SPARE_BYTES
            {
                self.unneeded.push(self.dedicated.remove(at).buffer);
                continue;
            }
            if spare {
                spare_bytes += dedicated.buffer.size();
            }
            at += 1;
        }
    }

    pub(crate) fn empty(&mut self, completed: u64) {
        self.chunks.retain(|chunk| chunk.used_by > completed);
        self.dedicated
            .retain(|dedicated| dedicated.used_by > completed);
        self.unneeded.clear();
        self.coming.clear();
    }
}

fn split_longer_than(copies: &mut Vec<vk::BufferCopy>, most: vk::DeviceSize) {
    if copies.iter().all(|copy| copy.size <= most) {
        return;
    }
    let whole = std::mem::take(copies);
    for copy in whole {
        let mut done = 0;
        while done < copy.size {
            let size = (copy.size - done).min(most);
            copies.push(vk::BufferCopy {
                src_offset: copy.src_offset + done,
                dst_offset: copy.dst_offset + done,
                size,
            });
            done += size;
        }
    }
}

fn chunk_size(needed: vk::DeviceSize, largest: Option<vk::DeviceSize>) -> vk::DeviceSize {
    let grown = largest.map_or(MIN_CHUNK_SIZE, |largest| (largest * 2).min(MAX_CHUNK_SIZE));
    needed.next_multiple_of(MIN_CHUNK_SIZE).max(grown)
}

impl std::fmt::Debug for StagingBelt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StagingBelt").finish_non_exhaustive()
    }
}
