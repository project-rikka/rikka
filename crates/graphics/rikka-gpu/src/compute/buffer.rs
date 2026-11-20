use std::mem::ManuallyDrop;
use std::sync::Arc;

use ash::vk;

use crate::compute::context::Shared;
use crate::compute::lane::{Lane, Marks, Uses};
use crate::core::buffer::GpuBuffer;
use crate::core::memory::Placement;
use crate::error::{Error, Result};

#[derive(Clone)]
pub struct ComputeBuffer {
    pub(crate) inner: Arc<BufferInner>,
}

pub(crate) struct BufferInner {
    shared: Arc<Shared>,
    buffer: ManuallyDrop<GpuBuffer>,
    address: u64,
    size: u64,
    id: u64,
    uses: Uses,
}

const USAGE: vk::BufferUsageFlags = vk::BufferUsageFlags::from_raw(
    vk::BufferUsageFlags::STORAGE_BUFFER.as_raw()
        | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS.as_raw()
        | vk::BufferUsageFlags::INDIRECT_BUFFER.as_raw()
        | vk::BufferUsageFlags::TRANSFER_SRC.as_raw()
        | vk::BufferUsageFlags::TRANSFER_DST.as_raw(),
);

impl ComputeBuffer {
    pub(crate) fn device(shared: &Arc<Shared>, size: u64) -> Result<ComputeBuffer> {
        Self::allocate(shared, size, Placement::Device)
    }

    pub(crate) fn constants(shared: &Arc<Shared>, bytes: &[u8]) -> Result<ComputeBuffer> {
        let allocated = Self::allocate(shared, bytes.len() as u64, Placement::Upload)?;
        let mut inner = Arc::into_inner(allocated.inner).expect("new buffer has a single owner");
        let mapped = inner
            .buffer
            .mapped_slice_mut()
            .ok_or_else(|| Error::Unsupported("constant buffers must be host-visible".into()))?;
        mapped[..bytes.len()].copy_from_slice(bytes);
        Ok(ComputeBuffer {
            inner: Arc::new(inner),
        })
    }

    fn allocate(shared: &Arc<Shared>, size: u64, location: Placement) -> Result<ComputeBuffer> {
        if size == 0 {
            return Err(Error::Invalid(
                "compute buffer size must be non-zero".into(),
            ));
        }
        let size = size.next_multiple_of(4);
        let buffer = GpuBuffer::create(
            shared.device.clone(),
            "rikka.compute.buffer",
            size,
            USAGE,
            location,
            shared.shared_by,
        )?;
        let address = buffer.device_address();
        Ok(ComputeBuffer {
            inner: Arc::new(BufferInner {
                shared: Arc::clone(shared),
                buffer: ManuallyDrop::new(buffer),
                address,
                size,
                id: shared.allocate_id(),
                uses: Uses::default(),
            }),
        })
    }

    #[must_use]
    pub fn address(&self) -> u64 {
        self.inner.address
    }

    #[must_use]
    pub fn size(&self) -> u64 {
        self.inner.size
    }

    #[must_use]
    pub fn is_unique(&self) -> bool {
        Arc::strong_count(&self.inner) == 1
    }

    pub(crate) fn id(&self) -> u64 {
        self.inner.id
    }

    pub(crate) fn handle(&self) -> vk::Buffer {
        self.inner.buffer.handle()
    }

    pub fn belongs_to(&self, shared: &Arc<Shared>) -> bool {
        Arc::ptr_eq(&self.inner.shared, shared)
    }

    pub(crate) fn used_until(&self, lane: Lane, value: u64) {
        self.inner.uses.mark(lane, value);
    }

    pub(crate) fn uses(&self) -> Marks {
        self.inner.uses.marks()
    }
}

impl Drop for BufferInner {
    fn drop(&mut self) {
        let last = self.uses.marks();
        let buffer = unsafe { ManuallyDrop::take(&mut self.buffer) };
        self.shared.retire(last, Box::new(buffer));
    }
}

impl std::fmt::Debug for ComputeBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComputeBuffer")
            .field("size", &self.inner.size)
            .finish_non_exhaustive()
    }
}
