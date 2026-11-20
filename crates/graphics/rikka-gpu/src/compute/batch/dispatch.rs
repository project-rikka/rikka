use ash::vk;

use crate::compute::buffer::ComputeBuffer;
use crate::compute::foreign::ForeignImage;
use crate::compute::image::ComputeImage;
use crate::compute::table::PUSH_BYTES;

const PUSH: usize = PUSH_BYTES as usize;

#[derive(Clone, Copy)]
pub(super) struct Push {
    bytes: [u8; PUSH],
    pub(super) len: usize,
}

impl Push {
    const EMPTY: Push = Push {
        bytes: [0; PUSH],
        len: 0,
    };

    fn from_bytes(bytes: &[u8]) -> Push {
        let mut push = Push::EMPTY;
        let len = bytes.len().min(PUSH);
        push.bytes[..len].copy_from_slice(&bytes[..len]);
        push.len = bytes.len();
        push
    }

    pub(super) fn fits(&self) -> bool {
        self.len <= PUSH
    }

    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes[..self.len.min(PUSH)]
    }
}

#[derive(Debug, Clone)]
pub(super) enum Groups {
    Direct([u32; 3]),
    Indirect(ComputeBuffer, u64),
}

#[derive(Clone, Copy)]
pub(super) enum Launch {
    Direct([u32; 3]),
    Indirect(vk::Buffer, u64),
}

#[must_use]
#[derive(Clone)]
pub struct Dispatch {
    pub(super) groups: Groups,
    pub(super) push: Push,
    pub(super) images: Vec<(ComputeImage, bool)>,
    pub(super) buffers: Vec<(ComputeBuffer, bool)>,
    pub(super) foreign: Vec<(ForeignImage, bool)>,
}

impl Dispatch {
    pub fn new(groups: [u32; 3]) -> Dispatch {
        Dispatch::launched(Groups::Direct(groups))
    }

    pub fn indirect(counts: &ComputeBuffer, offset: u64) -> Dispatch {
        Dispatch::launched(Groups::Indirect(counts.clone(), offset))
    }

    fn launched(groups: Groups) -> Dispatch {
        Dispatch {
            groups,
            push: Push::EMPTY,
            images: Vec::new(),
            buffers: Vec::new(),
            foreign: Vec::new(),
        }
    }

    pub fn reads(mut self, image: &ComputeImage) -> Dispatch {
        self.images.push((image.clone(), false));
        self
    }

    pub fn writes(mut self, image: &ComputeImage) -> Dispatch {
        self.images.push((image.clone(), true));
        self
    }

    pub fn reads_buffer(mut self, buffer: &ComputeBuffer) -> Dispatch {
        self.buffers.push((buffer.clone(), false));
        self
    }

    pub fn writes_buffer(mut self, buffer: &ComputeBuffer) -> Dispatch {
        self.buffers.push((buffer.clone(), true));
        self
    }

    pub fn reads_foreign(mut self, image: &ForeignImage) -> Dispatch {
        self.foreign.push((image.clone(), false));
        self
    }

    pub fn writes_foreign(mut self, image: &ForeignImage) -> Dispatch {
        self.foreign.push((image.clone(), true));
        self
    }

    pub fn push(mut self, bytes: &[u8]) -> Dispatch {
        self.push = Push::from_bytes(bytes);
        self
    }
}

impl std::fmt::Debug for Dispatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dispatch")
            .field("groups", &self.groups)
            .field("push", &self.push.len)
            .field("images", &self.images.len())
            .field("buffers", &self.buffers.len())
            .field("foreign", &self.foreign.len())
            .finish()
    }
}
