use std::path::Path;
use std::sync::Arc;

use rikka_image::{Compression, Error, Filter, Image, Png};
use rikka_ui::ImageSource;

const COMPRESSION: Compression = Compression::Fast;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Within {
    Cover,
    Contain,
}

#[derive(Clone, Debug)]
pub struct Developed {
    pub width: u32,
    pub height: u32,
    pixels: Arc<[u8]>,
}

impl Developed {
    #[must_use]
    pub fn source(&self) -> ImageSource {
        ImageSource::Rgba {
            width: self.width,
            height: self.height,
            pixels: Arc::clone(&self.pixels),
        }
    }
}

pub fn png(picture: &Image) -> Result<Vec<u8>, Error> {
    picture.write(Png).compression(COMPRESSION).to_bytes()
}

pub fn develop(
    path: &Path,
    bounds: Option<(u32, u32)>,
    within: Within,
) -> Result<Developed, String> {
    let failed = |error| format!("{}: {error}", path.display());
    let image = Image::open(path).map_err(failed)?;
    let image = match bounds.and_then(|bounds| shrunk(image.dimensions(), bounds, within)) {
        Some((width, height)) => image
            .resize_exact(width, height, Filter::Lanczos3)
            .map_err(failed)?,
        None => image,
    };
    let (width, height) = image.dimensions();
    Ok(Developed {
        width,
        height,
        pixels: Arc::from(image.into_rgba8()),
    })
}

fn shrunk(
    (width, height): (u32, u32),
    (wide, tall): (u32, u32),
    within: Within,
) -> Option<(u32, u32)> {
    if width == 0 || height == 0 {
        return None;
    }
    let across = f64::from(wide) / f64::from(width);
    let down = f64::from(tall) / f64::from(height);
    let factor = match within {
        Within::Cover => across.max(down),
        Within::Contain => across.min(down),
    };
    let side = |length: u32| {
        let scaled = f64::from(length) * factor;
        let rounded = match within {
            Within::Cover => scaled.ceil(),
            Within::Contain => scaled.floor(),
        };
        (rounded as u32).max(1)
    };
    (factor < 1.0).then(|| (side(width), side(height)))
}
