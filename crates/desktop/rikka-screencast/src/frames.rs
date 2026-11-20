use rikka_sync::{Mutex, MutexGuard};
use std::os::fd::{AsFd, BorrowedFd};
use std::time::Instant;

use rikka_gpu::{DmabufPlane, DmabufSpec};
use rikka_worker::{EventFd, EventNotifier, event_fd};

use crate::capture::held::{Fds, Held};

const SPARES: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelOrder {
    Bgra,
    Rgba,
}

#[derive(Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub captured: Instant,
    pub picture: Picture,
}

#[derive(Debug)]
pub enum Picture {
    Pixels(Pixels),
    Dmabuf(Dmabuf),
}

#[derive(Clone, PartialEq, Eq)]
pub struct Pixels {
    pub order: PixelOrder,
    pub stride: usize,
    pub bytes: Vec<u8>,
}

impl std::fmt::Debug for Pixels {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Pixels")
            .field("order", &self.order)
            .field("stride", &self.stride)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BufferKey {
    pub(crate) generation: u64,
    pub(crate) buffer: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Plane {
    pub(crate) offset: u32,
    pub(crate) stride: u32,
}

pub struct Dmabuf {
    pub(crate) key: BufferKey,
    pub(crate) fourcc: u32,
    pub(crate) modifier: u64,
    pub(crate) fds: Fds,
    pub(crate) planes: Vec<Plane>,
    pub(crate) _held: Held,
}

impl Dmabuf {
    #[must_use]
    pub fn key(&self) -> BufferKey {
        self.key
    }

    pub fn with_spec<T>(
        &self,
        (width, height): (u32, u32),
        use_spec: impl FnOnce(&DmabufSpec<'_>) -> T,
    ) -> T {
        let planes: Vec<DmabufPlane<'_>> = self
            .planes
            .iter()
            .zip(self.fds.iter())
            .map(|(plane, fd)| DmabufPlane {
                offset: plane.offset,
                stride: plane.stride,
                fd: fd.as_fd(),
            })
            .collect();
        use_spec(&DmabufSpec {
            width,
            height,
            fourcc: self.fourcc,
            modifier: self.modifier,
            planes: &planes,
        })
    }
}

impl std::fmt::Debug for Dmabuf {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Dmabuf")
            .field("key", &self.key)
            .field("fourcc", &self.fourcc)
            .field("modifier", &self.modifier)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum NextFrame {
    Frame(Frame),
    Pending,
    Ended(Option<String>),
}

#[derive(Default)]
struct Slot {
    frame: Option<Frame>,
    ended: Option<Option<String>>,
    spare_buffers: Vec<Vec<u8>>,
}

pub(crate) struct Latest {
    slot: Mutex<Slot>,
    bell: EventNotifier,
    arrived: EventFd,
}

impl Latest {
    pub(crate) fn new() -> std::io::Result<Latest> {
        let (bell, arrived) = event_fd()?;
        Ok(Latest {
            slot: Mutex::new(Slot::default()),
            bell,
            arrived,
        })
    }

    pub(crate) fn publish(&self, frame: Frame) {
        let mut slot = self.slot();
        let overtaken = slot.frame.replace(frame);
        if let Some(Frame {
            picture: Picture::Pixels(pixels),
            ..
        }) = overtaken
            && slot.spare_buffers.len() < SPARES
        {
            slot.spare_buffers.push(pixels.bytes);
        }
        drop(slot);
        self.ring();
    }

    pub(crate) fn end(&self, reason: Option<String>) {
        let mut slot = self.slot();
        slot.ended.get_or_insert(reason);
        let held = slot.frame.take();
        drop(slot);
        drop(held);
        self.ring();
    }

    pub(crate) fn has_ended(&self) -> bool {
        self.slot().ended.is_some()
    }

    pub(crate) fn take_buffer(&self, length: usize) -> Vec<u8> {
        let mut pixels = self.slot().spare_buffers.pop().unwrap_or_default();
        pixels.clear();
        pixels.reserve(length);
        pixels
    }

    pub(crate) fn recycle(&self, pixels: Vec<u8>) {
        let mut slot = self.slot();
        if slot.spare_buffers.len() < SPARES {
            slot.spare_buffers.push(pixels);
        }
    }

    pub(crate) fn next(&self) -> NextFrame {
        self.arrived.drain();
        let mut slot = self.slot();
        if let Some(frame) = slot.frame.take() {
            return NextFrame::Frame(frame);
        }
        match &slot.ended {
            Some(reason) => NextFrame::Ended(reason.clone()),
            None => NextFrame::Pending,
        }
    }

    fn ring(&self) {
        if let Err(error) = self.bell.notify() {
            tracing::warn!("cannot signal a captured frame: {error}");
        }
    }

    fn slot(&self) -> MutexGuard<'_, Slot> {
        self.slot.lock()
    }
}

impl AsFd for Latest {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.arrived.as_fd()
    }
}
