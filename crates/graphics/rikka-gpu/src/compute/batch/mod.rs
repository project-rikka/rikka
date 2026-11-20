mod command;
mod dispatch;
mod record;
mod regions;
mod submit;

use std::sync::Arc;

use ash::vk;
use rustc_hash::FxHashSet;

use crate::compute::buffer::ComputeBuffer;
use crate::compute::context::Compute;
use crate::compute::foreign::ForeignImage;
use crate::compute::image::ComputeImage;
use crate::compute::kernel::Kernel;
use crate::compute::pool::{Use, fitting};
use crate::compute::table::PUSH_BYTES;
use crate::compute::ticket::{Download, DownloadSlot};
use crate::compute::timing::{Clock, Timing};
use crate::core::buffer::GpuBuffer;
use crate::core::image::color_subresource_layers;
use crate::core::memory::Placement;
use crate::error::{Error, Result};

use command::{Command, Key};
use dispatch::{Groups, Launch};
use regions::{
    buffer_image_region, extent3, fits, foreign, offset, outside, outside_buffer, within,
};

pub use dispatch::Dispatch;

#[must_use = "a batch does nothing until it is submitted"]
pub struct Batch {
    compute: Compute,
    commands: Vec<Command>,
    images: Vec<ComputeImage>,
    foreign: Vec<ForeignImage>,
    buffers: Vec<ComputeBuffer>,
    bound_keys: FxHashSet<Key>,
    kernels: Vec<Kernel>,
    staging: Vec<GpuBuffer>,
    landings: Vec<Arc<DownloadSlot>>,
    scopes: Vec<&'static str>,
    clock: Option<Arc<Clock>>,
    failed: Option<Error>,
}

impl Batch {
    pub(crate) fn new(compute: Compute) -> Batch {
        Batch {
            compute,
            commands: Vec::new(),
            images: Vec::new(),
            buffers: Vec::new(),
            foreign: Vec::new(),
            bound_keys: FxHashSet::default(),
            kernels: Vec::new(),
            staging: Vec::new(),
            landings: Vec::new(),
            scopes: Vec::new(),
            clock: None,
            failed: None,
        }
    }

    pub fn scope(&mut self, name: &'static str) {
        self.commands.push(Command::Stamp {
            index: self.scopes.len() as u32,
        });
        self.scopes.push(name);
    }

    pub fn timing(&mut self) -> Timing {
        let shared = &self.compute.shared;
        let lane = self.compute.lane;
        let clock = self
            .clock
            .get_or_insert_with(|| Clock::new(shared.device.clone(), shared.line(lane).family));
        Timing {
            shared: Arc::clone(shared),
            lane,
            clock: Arc::clone(clock),
        }
    }

    pub fn dispatch(&mut self, kernel: &Kernel, dispatch: Dispatch) {
        if !dispatch.push.fits() {
            self.fail(Error::Invalid(format!(
                "push constants of {} bytes exceed the {PUSH_BYTES}-byte limit",
                dispatch.push.len
            )));
            return;
        }
        if !kernel.belongs_to(&self.compute.shared) {
            self.fail(foreign("kernel"));
            return;
        }
        let mut reads = Vec::with_capacity(dispatch.images.len() + dispatch.buffers.len() + 1);
        let mut writes = Vec::new();
        let launch = match &dispatch.groups {
            Groups::Direct(groups) => {
                let most = self
                    .compute
                    .shared
                    .device
                    .limits()
                    .max_compute_work_group_count;
                if let Some(axis) = (0..3).find(|axis| groups[*axis] > most[*axis]) {
                    self.fail(Error::Invalid(format!(
                        "dispatch of {} workgroups on axis {axis} exceeds the device limit of {}",
                        groups[axis], most[axis]
                    )));
                    return;
                }
                Launch::Direct(*groups)
            }
            Groups::Indirect(counts, offset) => {
                let fits = offset.is_multiple_of(4)
                    && offset
                        .checked_add(12)
                        .is_some_and(|end| end <= counts.size());
                if !fits {
                    self.fail(Error::Invalid(format!(
                        "indirect dispatch arguments at {offset} exceed a buffer of {} bytes",
                        counts.size()
                    )));
                    return;
                }
                let Some(key) = self.buffer(counts) else {
                    return;
                };
                reads.push(key);
                Launch::Indirect(counts.handle(), *offset)
            }
        };
        for (image, written) in &dispatch.images {
            let Some(key) = self.image(image) else {
                return;
            };
            match written {
                true => writes.push(key),
                false => reads.push(key),
            }
        }
        for (image, written) in &dispatch.foreign {
            let Some(key) = self.foreign_image(image) else {
                return;
            };
            match written {
                true => writes.push(key),
                false => reads.push(key),
            }
        }
        for (buffer, written) in &dispatch.buffers {
            let Some(key) = self.buffer(buffer) else {
                return;
            };
            match written {
                true => writes.push(key),
                false => reads.push(key),
            }
        }
        if self
            .kernels
            .last()
            .is_none_or(|last| last.pipeline() != kernel.pipeline())
        {
            self.kernels.push(kernel.clone());
        }
        self.commands.push(Command::Dispatch {
            pipeline: kernel.pipeline(),
            launch,
            push: dispatch.push,
            reads,
            writes,
        });
    }

    pub fn upload(
        &mut self,
        image: &ComputeImage,
        origin: [u32; 2],
        extent: [u32; 2],
        bytes: &[u8],
    ) {
        let row = u64::from(extent[0]) * u64::from(image.format().bytes_per_pixel());
        let expected = row * u64::from(extent[1]);
        if bytes.len() as u64 != expected {
            self.fail(Error::Invalid(format!(
                "{}x{} {:?} upload needs {expected} bytes, got {}",
                extent[0],
                extent[1],
                image.format(),
                bytes.len()
            )));
            return;
        }
        self.upload_rows(image, origin, extent, bytes, row as usize);
    }

    pub fn upload_rows(
        &mut self,
        image: &ComputeImage,
        origin: [u32; 2],
        extent: [u32; 2],
        bytes: &[u8],
        stride: usize,
    ) {
        let Some(key) = self.image(image) else {
            return;
        };
        if !fits(image.extent(), origin, extent) {
            self.fail(outside(image.extent(), origin, extent));
            return;
        }
        let row = extent[0] as usize * image.format().bytes_per_pixel() as usize;
        let rows = extent[1] as usize;
        let needed = stride
            .checked_mul(rows - 1)
            .and_then(|start| start.checked_add(row));
        if stride < row || needed.is_none_or(|needed| bytes.len() < needed) {
            self.fail(Error::Invalid(format!(
                "{}x{} {:?} upload needs {rows} rows of {row} bytes with stride {stride}, got {} bytes",
                extent[0],
                extent[1],
                image.format(),
                bytes.len()
            )));
            return;
        }
        let packed = stride == row;
        let Some(staging) = self.stage_with(row * rows, |mapped| match packed {
            true => mapped.copy_from_slice(&bytes[..row * rows]),
            false => {
                for (line, into) in mapped.chunks_exact_mut(row).enumerate() {
                    let start = line * stride;
                    into.copy_from_slice(&bytes[start..start + row]);
                }
            }
        }) else {
            return;
        };
        self.commands.push(Command::Upload {
            image: image.handle(),
            staging,
            region: buffer_image_region(origin, extent),
            key,
        });
    }

    pub fn copy(
        &mut self,
        from: &ComputeImage,
        from_origin: [u32; 2],
        to: &ComputeImage,
        to_origin: [u32; 2],
        extent: [u32; 2],
    ) {
        let (Some(read), Some(write)) = (self.image(from), self.image(to)) else {
            return;
        };
        if !fits(from.extent(), from_origin, extent) {
            self.fail(outside(from.extent(), from_origin, extent));
            return;
        }
        if !fits(to.extent(), to_origin, extent) {
            self.fail(outside(to.extent(), to_origin, extent));
            return;
        }
        if from.format().bytes_per_pixel() != to.format().bytes_per_pixel() {
            self.fail(Error::Invalid(format!(
                "cannot copy {:?} into {:?}",
                from.format(),
                to.format()
            )));
            return;
        }
        let region = vk::ImageCopy::default()
            .src_subresource(color_subresource_layers())
            .src_offset(offset(from_origin))
            .dst_subresource(color_subresource_layers())
            .dst_offset(offset(to_origin))
            .extent(extent3(extent));
        self.commands.push(Command::Copy {
            from: from.handle(),
            to: to.handle(),
            region,
            read,
            write,
        });
    }

    pub fn clear(&mut self, image: &ComputeImage, color: [f32; 4]) {
        let Some(key) = self.image(image) else {
            return;
        };
        self.commands.push(Command::Clear {
            image: image.handle(),
            range: image.range(),
            color,
            key,
        });
    }

    pub fn fill(&mut self, buffer: &ComputeBuffer, value: u32) {
        self.push_fill(buffer, 0, vk::WHOLE_SIZE, value);
    }

    pub fn fill_range(&mut self, buffer: &ComputeBuffer, at: u64, size: u64, value: u32) {
        if !within(buffer, at, size) || !at.is_multiple_of(4) || !size.is_multiple_of(4) {
            self.fail(Error::Invalid(format!(
                "fill of {size} bytes at {at} must be word-aligned within a buffer of {} bytes",
                buffer.size()
            )));
            return;
        }
        self.push_fill(buffer, at, size, value);
    }

    fn push_fill(&mut self, buffer: &ComputeBuffer, at: u64, size: u64, value: u32) {
        let Some(key) = self.buffer(buffer) else {
            return;
        };
        self.commands.push(Command::Fill {
            buffer: buffer.handle(),
            at,
            size,
            value,
            key,
        });
    }

    pub fn write(&mut self, buffer: &ComputeBuffer, at: u64, bytes: &[u8]) {
        let Some(key) = self.buffer(buffer) else {
            return;
        };
        if at
            .checked_add(bytes.len() as u64)
            .is_none_or(|end| end > buffer.size())
            || bytes.is_empty()
        {
            self.fail(Error::Invalid(format!(
                "{} bytes at offset {at} exceed a buffer of {} bytes",
                bytes.len(),
                buffer.size()
            )));
            return;
        }
        let Some(staging) = self.stage(bytes) else {
            return;
        };
        self.commands.push(Command::WriteBuffer {
            staging,
            buffer: buffer.handle(),
            region: vk::BufferCopy::default()
                .dst_offset(at)
                .size(bytes.len() as u64),
            key,
        });
    }

    pub fn download(
        &mut self,
        image: &ComputeImage,
        origin: [u32; 2],
        extent: [u32; 2],
    ) -> Download {
        let size = u64::from(extent[0])
            * u64::from(extent[1])
            * u64::from(image.format().bytes_per_pixel());
        let download_slot = self.download_slot(size);
        let key = self.image(image);
        match (
            key,
            download_slot.handle(),
            fits(image.extent(), origin, extent),
        ) {
            (Some(key), Some(into), true) => self.commands.push(Command::ImageOut {
                image: image.handle(),
                download_slot: into,
                region: buffer_image_region(origin, extent),
                key,
            }),
            (Some(_), Some(_), false) => self.fail(outside(image.extent(), origin, extent)),
            _ => {}
        }
        Download { download_slot }
    }

    pub fn download_buffer(&mut self, buffer: &ComputeBuffer) -> Download {
        self.download_range(buffer, 0, buffer.size())
    }

    pub fn download_range(&mut self, buffer: &ComputeBuffer, at: u64, size: u64) -> Download {
        let download_slot = self.download_slot(size);
        if !within(buffer, at, size) {
            self.fail(outside_buffer(buffer, at, size));
            return Download { download_slot };
        }
        if let (Some(key), Some(into)) = (self.buffer(buffer), download_slot.handle()) {
            self.commands.push(Command::BufferOut {
                buffer: buffer.handle(),
                download_slot: into,
                region: vk::BufferCopy::default().src_offset(at).size(size),
                key,
            });
        }
        Download { download_slot }
    }

    pub fn copy_buffer(
        &mut self,
        from: &ComputeBuffer,
        from_at: u64,
        to: &ComputeBuffer,
        to_at: u64,
        size: u64,
    ) {
        let (Some(read), Some(write)) = (self.buffer(from), self.buffer(to)) else {
            return;
        };
        if !within(from, from_at, size) {
            self.fail(outside_buffer(from, from_at, size));
            return;
        }
        if !within(to, to_at, size) {
            self.fail(outside_buffer(to, to_at, size));
            return;
        }
        let overlapping = from.id() == to.id() && from_at < to_at + size && to_at < from_at + size;
        if overlapping {
            self.fail(Error::Invalid(
                "buffer copy source and destination overlap".into(),
            ));
            return;
        }
        self.commands.push(Command::CopyBuffer {
            from: from.handle(),
            to: to.handle(),
            region: vk::BufferCopy::default()
                .src_offset(from_at)
                .dst_offset(to_at)
                .size(size),
            read,
            write,
        });
    }

    fn image(&mut self, image: &ComputeImage) -> Option<Key> {
        if !image.belongs_to(&self.compute.shared) {
            self.fail(foreign("image"));
            return None;
        }
        if self.bound_keys.insert(Key::Slot(image.slot())) {
            self.images.push(image.clone());
        }
        Some(Key::Image(image.id()))
    }

    fn foreign_image(&mut self, image: &ForeignImage) -> Option<Key> {
        if !image.belongs_to(&self.compute.shared) {
            self.fail(foreign("foreign image"));
            return None;
        }
        let key = Key::Image(image.id());
        if self.bound_keys.insert(key) {
            self.foreign.push(image.clone());
        }
        Some(key)
    }

    fn buffer(&mut self, buffer: &ComputeBuffer) -> Option<Key> {
        if !buffer.belongs_to(&self.compute.shared) {
            self.fail(foreign("buffer"));
            return None;
        }
        let key = Key::Buffer(buffer.id());
        if self.bound_keys.insert(key) {
            self.buffers.push(buffer.clone());
        }
        Some(key)
    }

    fn stage(&mut self, bytes: &[u8]) -> Option<vk::Buffer> {
        self.stage_with(bytes.len(), |mapped| mapped.copy_from_slice(bytes))
    }

    fn stage_with(&mut self, size: usize, fill: impl FnOnce(&mut [u8])) -> Option<vk::Buffer> {
        let reused = self.reuse_or_create(Use::Staging, size as u64);
        let mut buffer = match reused {
            Ok(buffer) => buffer,
            Err(error) => {
                self.fail(error);
                return None;
            }
        };
        let Some(mapped) = buffer.mapped_slice_mut() else {
            self.fail(Error::Unsupported(
                "staging memory is not host-visible".into(),
            ));
            return None;
        };
        fill(&mut mapped[..size]);
        let handle = buffer.handle();
        self.staging.push(buffer);
        Some(handle)
    }

    fn download_slot(&mut self, size: u64) -> Arc<DownloadSlot> {
        let shared = &self.compute.shared;
        let lane = self.compute.lane;
        let reused = self.reuse_or_create(Use::DownloadSlot, size.max(4));
        let size = usize::try_from(size).unwrap_or(usize::MAX);
        match reused {
            Ok(buffer) => {
                let download_slot = DownloadSlot::new(shared, lane, Some(buffer), size);
                self.landings.push(Arc::clone(&download_slot));
                download_slot
            }
            Err(error) => {
                let download_slot = DownloadSlot::new(shared, lane, None, size);
                self.fail(error);
                download_slot
            }
        }
    }

    fn reuse_or_create(&self, used: Use, size: u64) -> Result<GpuBuffer> {
        let shared = &self.compute.shared;
        let finished = shared.finished();
        let pooled = shared.state.lock().pool.take_buffer(used, size, finished);
        if let Some(buffer) = pooled {
            return Ok(buffer);
        }
        let (name, usage, placement) = match used {
            Use::Staging => (
                "rikka.compute.staging",
                vk::BufferUsageFlags::TRANSFER_SRC,
                Placement::Staging,
            ),
            Use::DownloadSlot => (
                "rikka.compute.download",
                vk::BufferUsageFlags::TRANSFER_DST,
                Placement::Readback,
            ),
        };
        GpuBuffer::create(
            shared.device.clone(),
            name,
            fitting(size),
            usage,
            placement,
            shared.shared_by,
        )
    }

    pub fn fail(&mut self, error: Error) {
        self.failed.get_or_insert(error);
    }
}

impl std::fmt::Debug for Batch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Batch")
            .field("commands", &self.commands.len())
            .field("lane", &self.compute.lane)
            .finish_non_exhaustive()
    }
}
