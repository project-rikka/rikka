mod error;
mod read;

pub use error::{Error, Result};
pub use read::{Info, info, still, still_with_max_size};
pub use rikka_media_format::{Detail, Extent, Format, Kind};
pub use rikka_video::PixelSize;
