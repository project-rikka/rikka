mod exif;
mod jpeg;
mod png;
mod webp;

use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::error::ConvertError;
use crate::target::extension_of;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Malformed;

pub(crate) fn strip(source: &Path, draft: &Path) -> Result<(), ConvertError> {
    let bytes = std::fs::read(source)?;
    let stripped = if bytes.starts_with(&[0xFF, 0xD8]) {
        jpeg::strip(&bytes)
    } else if bytes.starts_with(png::SIGNATURE) {
        png::strip(&bytes)
    } else if webp::is_webp(&bytes) {
        webp::strip(&bytes)
    } else {
        return Err(ConvertError::Other(format!(
            "{} is not a JPEG, PNG or WebP picture",
            source.display()
        )));
    }
    .map_err(|Malformed| ConvertError::Other(format!("{} is damaged", source.display())))?;
    let mut file = File::create(draft)?;
    file.write_all(&stripped)?;
    file.sync_all()?;
    Ok(())
}

pub(crate) fn can_strip(name: &str) -> bool {
    let extension = extension_of(name);
    ["jpg", "jpeg", "jpe", "png", "webp"]
        .iter()
        .any(|known| extension.eq_ignore_ascii_case(known))
}
