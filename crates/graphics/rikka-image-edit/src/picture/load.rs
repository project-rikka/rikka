use std::sync::Arc;

use rikka_gpu::ImageFormat;
use rikka_image::{ColorType, Image};
use rikka_types::{Color, Source};

use crate::color::Profile;
use crate::error::{Error, Result};
use crate::imaging::Imaging;
use crate::picture::{Depth, Frame, Mode, Picture};
use crate::plane::Plane;
use crate::work::Work;

impl Imaging {
    pub fn open<'a>(&self, source: impl Into<Source<'a>>) -> Result<Picture> {
        let image = rikka_image::read(source).decode()?;
        self.picture(&image)
    }

    pub fn picture(&self, image: &Image) -> Result<Picture> {
        let color = image.color();
        let depth = match color {
            ColorType::Rgb32F | ColorType::Rgba32F => Depth::Float,
            _ if color.bits_per_channel() > 8 => Depth::Sixteen,
            _ => Depth::Eight,
        };
        let mode = match image.is_grayscale() {
            true => Mode::Gray,
            false => Mode::Rgb,
        };
        let profile = image.icc_profile().and_then(|bytes| {
            match Profile::from_icc(Arc::<[u8]>::from(bytes)) {
                Ok(profile) => Some(profile),
                Err(error) => {
                    tracing::warn!("ignoring invalid colour profile: {error}");
                    None
                }
            }
        });
        let format = depth.storage(self.formats());
        let stored = stored(format);
        let converted = (image.color() != stored).then(|| image.convert(stored));
        let pixels = converted.as_ref().unwrap_or(image).as_bytes();
        let (width, height) = image.dimensions();
        let plane = Plane::new(self, width, height, format)?;
        let mut work = Work::new(self);
        upload(&mut work, &plane, pixels);
        work.finish()?;
        Ok(Picture::from_frame(
            self,
            Frame {
                plane,
                depth,
                mode,
                has_alpha: image.has_alpha(),
                space: self.space(profile.as_ref()),
            },
        ))
    }

    pub fn upload_rgba8(&self, width: u32, height: u32, pixels: &[u8]) -> Result<Picture> {
        let expected = u64::from(width) * u64::from(height) * 4;
        if pixels.len() as u64 != expected {
            return Err(Error::Invalid(format!(
                "{width}×{height} RGBA8 picture needs {expected} bytes, got {}",
                pixels.len()
            )));
        }
        let plane = Plane::new(self, width, height, ImageFormat::Rgba8)?;
        let mut work = Work::new(self);
        upload(&mut work, &plane, pixels);
        work.finish()?;
        Ok(Picture::from_frame(
            self,
            Frame {
                plane,
                depth: Depth::Eight,
                mode: Mode::Rgb,
                has_alpha: true,
                space: self.space(None),
            },
        ))
    }

    pub fn blank(&self, width: u32, height: u32, depth: Depth, fill: Color) -> Result<Picture> {
        let format = depth.storage(self.formats());
        let plane = Plane::new(self, width, height, format)?;
        let mut work = Work::new(self);
        let color = [fill.r, fill.g, fill.b, fill.a];
        for tile in plane.tiles() {
            work.batch().clear(tile, color);
        }
        work.finish()?;
        Ok(Picture::from_frame(
            self,
            Frame {
                plane,
                depth,
                mode: Mode::Rgb,
                has_alpha: fill.a < 1.0,
                space: self.space(None),
            },
        ))
    }
}

fn stored(format: ImageFormat) -> ColorType {
    match format {
        ImageFormat::Rgba8 => ColorType::Rgba8,
        ImageFormat::Rgba16 => ColorType::Rgba16,
        _ => ColorType::Rgba32F,
    }
}

pub(crate) fn upload(work: &mut Work, plane: &Plane, pixels: &[u8]) {
    let pixel = plane.format().bytes_per_pixel() as usize;
    let stride = plane.width() as usize * pixel;
    for ([left, top, width, height], tile) in plane.tiles_with_areas() {
        let first = top as usize * stride + left as usize * pixel;
        work.batch()
            .upload_rows(tile, [0, 0], [width, height], &pixels[first..], stride);
    }
}
