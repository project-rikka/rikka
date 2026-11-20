use std::path::Path;

use crate::error::{Error, Result};

const THUMBNAIL_SIDE: u32 = 128;

pub(crate) fn read_rgba8(path: &Path) -> Result<Vec<u8>> {
    let picture = rikka_media::still_with_max_size(
        path,
        rikka_media::PixelSize::new(THUMBNAIL_SIDE, THUMBNAIL_SIDE),
    )
    .map_err(|source| Error::Decode {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(picture.into_rgba8())
}
