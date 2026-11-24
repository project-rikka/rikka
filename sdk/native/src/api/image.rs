use std::path::Path;
use std::sync::Arc;

use rikka_ui::ImageSource;

use crate::abi::violation;

#[derive(Debug)]
enum Picture {
    File(Arc<Path>),
    Encoded(Arc<[u8]>),
    Rgba {
        width: u32,
        height: u32,
        pixels: Arc<[u8]>,
    },
}

#[derive(Debug)]
pub(crate) struct Image {
    picture: Picture,
}

impl Image {
    pub(crate) fn rgba(width: u32, height: u32, pixels: Arc<[u8]>) -> Arc<Image> {
        Arc::new(Image {
            picture: Picture::Rgba {
                width,
                height,
                pixels,
            },
        })
    }

    pub(crate) fn source(&self) -> ImageSource {
        match &self.picture {
            Picture::File(path) => ImageSource::file(Arc::clone(path)),
            Picture::Encoded(bytes) => ImageSource::Bytes(Arc::clone(bytes)),
            Picture::Rgba {
                width,
                height,
                pixels,
            } => ImageSource::Rgba {
                width: *width,
                height: *height,
                pixels: Arc::clone(pixels),
            },
        }
    }
}

pub(crate) fn image_new_file(path: &str) -> Arc<Image> {
    if path.is_empty() {
        violation("the path is empty");
    }
    Arc::new(Image {
        picture: Picture::File(Arc::from(Path::new(path))),
    })
}

pub(crate) fn image_new_encoded(data: &[u8]) -> Arc<Image> {
    Arc::new(Image {
        picture: Picture::Encoded(Arc::from(data)),
    })
}

pub(crate) fn image_new_rgba(width: u32, height: u32, pixels: &[u8]) -> Arc<Image> {
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|count| count.checked_mul(4));
    if width == 0 || height == 0 || expected != Some(pixels.len()) {
        violation(format!(
            "{} bytes are not {width} x {height} RGBA8 pixels",
            pixels.len()
        ));
    }
    Image::rgba(width, height, Arc::from(pixels))
}
