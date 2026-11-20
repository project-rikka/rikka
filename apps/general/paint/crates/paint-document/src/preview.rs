use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::sync::LazyLock;

use rikka_image::{Image, ImageLoader};
use rikka_psd::{FileHeader, Preamble, ThumbnailFormat};

use crate::thumbnail::Thumbnail;

const PREVIEW_THREADS: usize = 2;

static LOADER: LazyLock<ImageLoader> = LazyLock::new(|| ImageLoader::with_threads(PREVIEW_THREADS));

#[must_use]
pub fn preview(path: &Path, longest: u32) -> Option<Thumbnail> {
    let mut file = BufReader::new(File::open(path).ok()?);
    let mut signature = [0u8; FileHeader::SIGNATURE.len()];
    file.read_exact(&mut signature).ok()?;
    let image = match FileHeader::is_photoshop(&signature) {
        true => photoshop(&mut signature.chain(file))?,
        false => LOADER
            .read(path)
            .thumbnail(longest, longest)
            .submit()
            .wait()
            .ok()?,
    };
    let image = image.thumbnail(longest, longest).ok()?;
    let (width, height) = image.dimensions();
    Some(Thumbnail {
        width,
        height,
        pixels: image.into_rgba8(),
    })
}

fn photoshop(source: &mut impl Read) -> Option<Image> {
    let preamble = Preamble::read(source).ok()?;
    let thumbnail = preamble.resources.thumbnail()?;
    match thumbnail.format {
        ThumbnailFormat::Jpeg => rikka_image::read(thumbnail.data.as_slice())
            .format(rikka_image::Format::Jpeg)
            .decode()
            .ok(),
        ThumbnailFormat::Raw => {
            let rgb = thumbnail.raw_rgb8()?;
            Image::from_rgb8(thumbnail.width, thumbnail.height, rgb).ok()
        }
    }
}
