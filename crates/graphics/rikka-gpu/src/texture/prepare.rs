use crate::error::{Error, Result};

use super::format::TextureFormat;
use super::pixels::Pixels;

pub fn prepared(pixels: Pixels) -> Result<Vec<u8>> {
    let Pixels {
        extent,
        format,
        mut data,
    } = pixels;
    if extent[0] == 0 || extent[1] == 0 {
        return Err(Error::Invalid(format!(
            "upload size {}x{} is empty",
            extent[0], extent[1]
        )));
    }
    validate_size(extent[0], extent[1], format, &data)?;
    if format.premultiplies() {
        crate::pixels::premultiply_rgba8(&mut data);
    }
    Ok(data)
}

fn validate_size(width: u32, height: u32, format: TextureFormat, data: &[u8]) -> Result<()> {
    let expected = width as usize * height as usize * format.bytes_per_texel() as usize;
    if data.len() != expected {
        return Err(Error::Invalid(format!(
            "texture data is {} bytes, expected {expected} ({width}x{height} {format:?})",
            data.len()
        )));
    }
    Ok(())
}
