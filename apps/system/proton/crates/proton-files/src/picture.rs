use std::path::Path;

use rikka_image::Jpeg;
use rikka_media::PixelSize;
use rikka_media_format::Kind;
use rikka_proton_drive::{THUMBNAIL_MAX_SIDE, THUMBNAIL_MAX_SIZE};

const QUALITIES: [u8; 4] = [70, 50, 30, 10];

#[must_use]
pub fn preview(path: &Path) -> Option<Vec<u8>> {
    if !matches!(
        rikka_media_format::kind(path),
        Some(Kind::Picture | Kind::Movie)
    ) {
        return None;
    }

    let max_size = PixelSize::new(THUMBNAIL_MAX_SIDE, THUMBNAIL_MAX_SIDE);
    let small = rikka_media::still_with_max_size(path, max_size).ok()?;
    QUALITIES.into_iter().find_map(|quality| {
        small
            .write(Jpeg)
            .quality(quality)
            .to_bytes()
            .ok()
            .filter(|encoded| encoded.len() <= THUMBNAIL_MAX_SIZE)
    })
}
