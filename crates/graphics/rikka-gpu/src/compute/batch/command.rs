use ash::vk;

use super::dispatch::{Launch, Push};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Key {
    Image(u64),
    Slot(u32),
    Buffer(u64),
}
pub(super) enum Command {
    Dispatch {
        pipeline: vk::Pipeline,
        launch: Launch,
        push: Push,
        reads: Vec<Key>,
        writes: Vec<Key>,
    },
    Upload {
        image: vk::Image,
        staging: vk::Buffer,
        region: vk::BufferImageCopy,
        key: Key,
    },
    Copy {
        from: vk::Image,
        to: vk::Image,
        region: vk::ImageCopy,
        read: Key,
        write: Key,
    },
    Clear {
        image: vk::Image,
        range: vk::ImageSubresourceRange,
        color: [f32; 4],
        key: Key,
    },
    Fill {
        buffer: vk::Buffer,
        at: u64,
        size: u64,
        value: u32,
        key: Key,
    },
    WriteBuffer {
        staging: vk::Buffer,
        buffer: vk::Buffer,
        region: vk::BufferCopy,
        key: Key,
    },
    ImageOut {
        image: vk::Image,
        download_slot: vk::Buffer,
        region: vk::BufferImageCopy,
        key: Key,
    },
    BufferOut {
        buffer: vk::Buffer,
        download_slot: vk::Buffer,
        region: vk::BufferCopy,
        key: Key,
    },
    CopyBuffer {
        from: vk::Buffer,
        to: vk::Buffer,
        region: vk::BufferCopy,
        read: Key,
        write: Key,
    },
    Stamp {
        index: u32,
    },
}

impl Command {
    pub(super) fn touches(&self) -> (&[Key], &[Key]) {
        match self {
            Command::Dispatch { reads, writes, .. } => (reads, writes),
            Command::Upload { key, .. }
            | Command::Clear { key, .. }
            | Command::Fill { key, .. }
            | Command::WriteBuffer { key, .. } => (&[], std::slice::from_ref(key)),
            Command::Copy { read, write, .. } | Command::CopyBuffer { read, write, .. } => {
                (std::slice::from_ref(read), std::slice::from_ref(write))
            }
            Command::ImageOut { key, .. } | Command::BufferOut { key, .. } => {
                (std::slice::from_ref(key), &[])
            }
            Command::Stamp { .. } => (&[], &[]),
        }
    }
}
