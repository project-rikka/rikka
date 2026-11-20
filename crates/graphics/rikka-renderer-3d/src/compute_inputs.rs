use std::sync::Arc;

use ash::vk;

use crate::draw::Frame;
use crate::error::{Error, Result};
use rikka_gpu::compute::{ComputeBuffer, Ticket};
use rikka_gpu::core::device::RenderDevice;

#[derive(Clone, Debug)]
pub(crate) struct ComputeSource {
    pub buffer: ComputeBuffer,
    pub ready: Ticket,
}

impl ComputeSource {
    pub(crate) fn new(buffer: &ComputeBuffer, ready: &Ticket) -> Self {
        Self {
            buffer: buffer.clone(),
            ready: ready.clone(),
        }
    }

    fn check(&self, what: &str) -> Result<()> {
        match self.buffer.belongs_to(&self.ready.shared) {
            true => Ok(()),
            false => Err(Error::Invalid(format!(
                "{what} buffer and ticket belong to different compute contexts"
            ))),
        }
    }
}

pub(crate) struct PerVertex<'a> {
    pub what: &'static str,
    pub sources: &'a [ComputeSource],
    pub stride: u64,
}

impl PerVertex<'_> {
    pub(crate) fn address(&self, source: u32, first: u32, vertices: u32) -> Result<u64> {
        let what = self.what;
        let Some(entry) = self.sources.get(source as usize) else {
            return Err(Error::Invalid(format!(
                "draw references {what} source {source} of {} in this frame",
                self.sources.len()
            )));
        };
        let end = (u64::from(first) + u64::from(vertices)) * self.stride;
        if end > entry.buffer.size() {
            return Err(Error::Invalid(format!(
                "mesh of {vertices} vertices reads {what} from vertex {first} and needs {end} \
                 bytes, but the buffer has {}",
                entry.buffer.size()
            )));
        }
        Ok(entry.buffer.address() + u64::from(first) * self.stride)
    }
}

pub(crate) struct ComputeInputs {
    pub buffers: Vec<ComputeBuffer>,
    waits: Vec<(vk::Semaphore, u64)>,
}

impl ComputeInputs {
    pub(crate) fn collect(frame: &Frame, device: &Arc<RenderDevice>) -> Result<Self> {
        let mut inputs = Self {
            buffers: Vec::new(),
            waits: Vec::new(),
        };
        for layer in &frame.fluids {
            layer.check()?;
            inputs.take(&layer.ready, device)?;
            inputs.buffers.extend(layer.buffers().cloned());
        }
        for (sources, what) in [(&frame.wets, "wetness"), (&frame.drapes, "drape")] {
            for source in sources {
                source.check(what)?;
                inputs.take(&source.ready, device)?;
                inputs.buffers.push(source.buffer.clone());
            }
        }
        Ok(inputs)
    }

    fn take(&mut self, ready: &Ticket, device: &Arc<RenderDevice>) -> Result<()> {
        if !Arc::ptr_eq(&ready.shared.device, device) {
            return Err(Error::Invalid(
                "fluid, wetness or drape ticket belongs to another renderer's compute context"
                    .into(),
            ));
        }
        let wait = (ready.semaphore(), ready.value());
        if !self.waits.contains(&wait) {
            self.waits.push(wait);
        }
        Ok(())
    }

    pub(crate) fn waits(&self) -> impl Iterator<Item = vk::SemaphoreSubmitInfo<'static>> + '_ {
        self.waits.iter().map(|&(semaphore, value)| {
            vk::SemaphoreSubmitInfo::default()
                .semaphore(semaphore)
                .value(value)
                .stage_mask(
                    vk::PipelineStageFlags2::VERTEX_SHADER
                        | vk::PipelineStageFlags2::FRAGMENT_SHADER
                        | vk::PipelineStageFlags2::COMPUTE_SHADER,
                )
        })
    }
}
