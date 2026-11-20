use rikka_compositor_types::Format;

use crate::dmabuf::Dmabuf;
use crate::dumb::Mapped;
use crate::framebuffer::Framebuffer;
use crate::ids::FramebufferId;
use crate::ring::Ring;

#[derive(Debug)]
pub struct Swapchain {
    ring: Ring<Buffer>,
    format: Format,
}

#[derive(Debug)]
pub(crate) struct Buffer {
    pub(crate) framebuffer: Framebuffer,
    pub(crate) memory: Memory,
}

#[derive(Debug)]
pub(crate) enum Memory {
    Dmabuf(Dmabuf),
    Mapped(Mapped),
}

#[derive(Debug)]
pub struct Target {
    pub framebuffer: Framebuffer,
    pub age: u32,
}

impl Swapchain {
    pub(crate) fn new(buffers: Vec<Buffer>, format: Format) -> Self {
        Self {
            ring: Ring::new(buffers),
            format,
        }
    }

    pub fn acquire(&mut self) -> Option<Target> {
        let (buffer, age) = self
            .ring
            .acquire(|buffer| buffer.framebuffer.is_held_elsewhere())?;
        Some(Target {
            framebuffer: buffer.framebuffer.clone(),
            age,
        })
    }

    pub fn forget_ages(&mut self) {
        self.ring.forget_ages();
    }

    pub fn dmabuf(&self, framebuffer: FramebufferId) -> Option<&Dmabuf> {
        self.ring.items().find_map(|buffer| match &buffer.memory {
            Memory::Dmabuf(dmabuf) if buffer.framebuffer.id() == framebuffer => Some(dmabuf),
            _ => None,
        })
    }

    pub fn mapped(&mut self, framebuffer: FramebufferId) -> Option<&mut Mapped> {
        self.ring
            .items_mut()
            .find_map(|buffer| match &mut buffer.memory {
                Memory::Mapped(mapped) if buffer.framebuffer.id() == framebuffer => Some(mapped),
                _ => None,
            })
    }

    pub fn framebuffers(&self) -> impl Iterator<Item = &Framebuffer> {
        self.ring.items().map(|buffer| &buffer.framebuffer)
    }

    pub fn format(&self) -> Format {
        self.format
    }
}
