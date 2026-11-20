mod book;
mod copies;
mod created;
mod entry;
mod format;
mod lookup;
pub(crate) mod packed;
mod pending_upload;
mod pixels;
mod pool;
mod prepare;
mod ring;
mod rows;
mod source;
mod uploads;
mod usage;
mod writes;

use ash::vk;

pub(crate) use book::Kind;
pub use book::TextureBook;
pub(crate) use created::Created;
pub use format::TextureFormat;
pub use pixels::Pixels;
pub use pool::TexturePool;
pub use prepare::prepared;
pub(crate) use ring::RingSpan;
pub use ring::UploadRing;
pub use rows::{Rows, Staged, TexelRect};
pub use uploads::Upload;
pub use uploads::Uploads;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TextureId(u64);

pub const OUTPUT_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;

const PICTURE_USAGE: vk::ImageUsageFlags = vk::ImageUsageFlags::from_raw(
    vk::ImageUsageFlags::SAMPLED.as_raw()
        | vk::ImageUsageFlags::TRANSFER_DST.as_raw()
        | vk::ImageUsageFlags::TRANSFER_SRC.as_raw(),
);

const CANVAS_USAGE: vk::ImageUsageFlags = vk::ImageUsageFlags::from_raw(
    vk::ImageUsageFlags::SAMPLED.as_raw()
        | vk::ImageUsageFlags::TRANSFER_DST.as_raw()
        | vk::ImageUsageFlags::COLOR_ATTACHMENT.as_raw(),
);
