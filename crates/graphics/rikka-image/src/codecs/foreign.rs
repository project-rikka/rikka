use std::io::{BufRead, Seek, Write};
use std::sync::Arc;
use std::time::Duration;

use image::codecs::jpeg::{JpegDecoder, JpegEncoder};
use image::codecs::webp::WebPDecoder;
use image::metadata::{LoopCount, Orientation as ForeignOrientation};
use image::{AnimationDecoder, ExtendedColorType, ImageDecoder, ImageEncoder};

use crate::animated::{AnimatedImage, Blend, Compositor, Dispose, Patch};
use crate::codecs::{Limits, Probe, unsupported};
use crate::color_type::ColorType;
use crate::error::{Error, Result};
use crate::format::Format;
use crate::loops::Loops;
use crate::orientation::Orientation;
use crate::pixels::Pixels;
use crate::raster::Image;

impl From<image::ImageError> for Error {
    fn from(error: image::ImageError) -> Error {
        use image::ImageError;

        match error {
            ImageError::IoError(io) => Error::Io(io),
            ImageError::Decoding(error) => Error::Decoding(error.to_string()),
            ImageError::Encoding(error) => Error::Encoding(error.to_string()),
            ImageError::Limits(error) => Error::LimitsExceeded(error.to_string()),
            ImageError::Unsupported(error) => Error::Unsupported(error.to_string()),
            ImageError::Parameter(error) => Error::Encoding(error.to_string()),
        }
    }
}

impl From<LoopCount> for Loops {
    fn from(count: LoopCount) -> Loops {
        match count {
            LoopCount::Infinite => Loops::Infinite,
            LoopCount::Finite(count) => Loops::Finite(count.get()),
        }
    }
}

fn foreign_limits(limits: &Limits) -> image::Limits {
    let mut foreign = image::Limits::no_limits();
    foreign.max_alloc = limits.max_alloc;
    foreign.max_image_width = limits.max_dimensions.map(|(width, _)| width);
    foreign.max_image_height = limits.max_dimensions.map(|(_, height)| height);
    foreign
}

fn orientation(foreign: ForeignOrientation) -> Orientation {
    match foreign {
        ForeignOrientation::NoTransforms => Orientation::Upright,
        ForeignOrientation::Rotate90 => Orientation::Rotate90,
        ForeignOrientation::Rotate180 => Orientation::Rotate180,
        ForeignOrientation::Rotate270 => Orientation::Rotate270,
        ForeignOrientation::FlipHorizontal => Orientation::FlipHorizontal,
        ForeignOrientation::FlipVertical => Orientation::FlipVertical,
        ForeignOrientation::Rotate90FlipH => Orientation::Rotate90FlipHorizontal,
        ForeignOrientation::Rotate270FlipH => Orientation::Rotate270FlipHorizontal,
    }
}

fn color(foreign: image::ColorType, format: Format) -> Result<ColorType> {
    Ok(match foreign {
        image::ColorType::L8 => ColorType::L8,
        image::ColorType::La8 => ColorType::La8,
        image::ColorType::Rgb8 => ColorType::Rgb8,
        image::ColorType::Rgba8 => ColorType::Rgba8,
        image::ColorType::L16 => ColorType::L16,
        image::ColorType::La16 => ColorType::La16,
        image::ColorType::Rgb16 => ColorType::Rgb16,
        image::ColorType::Rgba16 => ColorType::Rgba16,
        image::ColorType::Rgb32F => ColorType::Rgb32F,
        image::ColorType::Rgba32F => ColorType::Rgba32F,
        _ => return Err(unsupported(format, "pixel layout")),
    })
}

fn extended(color: ColorType) -> ExtendedColorType {
    match color {
        ColorType::L8 => ExtendedColorType::L8,
        ColorType::La8 => ExtendedColorType::La8,
        ColorType::Rgb8 => ExtendedColorType::Rgb8,
        ColorType::Rgba8 => ExtendedColorType::Rgba8,
        ColorType::L16 => ExtendedColorType::L16,
        ColorType::La16 => ExtendedColorType::La16,
        ColorType::Rgb16 => ExtendedColorType::Rgb16,
        ColorType::Rgba16 => ExtendedColorType::Rgba16,
        ColorType::Rgb32F => ExtendedColorType::Rgb32F,
        ColorType::Rgba32F => ExtendedColorType::Rgba32F,
    }
}

pub(crate) struct Foreign<'r> {
    decoder: Box<dyn ImageDecoder + 'r>,
    format: Format,
    limits: image::Limits,
}

impl<'r> Foreign<'r> {
    pub(crate) fn open<R: BufRead + Seek + 'r>(
        format: Format,
        reader: R,
        limits: &Limits,
    ) -> Result<Foreign<'r>> {
        let limits = foreign_limits(limits);
        let mut decoder: Box<dyn ImageDecoder + 'r> = match format {
            Format::Jpeg => Box::new(JpegDecoder::new(reader)?),
            Format::WebP => Box::new(WebPDecoder::new(reader)?),
            other => return Err(unsupported(other, "decoding")),
        };
        decoder.set_limits(limits.clone())?;
        Ok(Foreign {
            decoder,
            format,
            limits,
        })
    }

    pub(crate) fn probe(&mut self) -> Result<(Probe, Orientation)> {
        let (width, height) = self.decoder.dimensions();
        Ok((
            Probe {
                width,
                height,
                color: color(self.decoder.color_type(), self.format)?,
            },
            orientation(self.decoder.orientation()?),
        ))
    }

    pub(crate) fn decode(mut self) -> Result<(Image, Orientation)> {
        let profile = self.decoder.icc_profile().ok().flatten().map(Arc::from);
        let (probe, orientation) = self.probe()?;
        self.limits.reserve(self.decoder.total_bytes())?;
        self.decoder.set_limits(self.limits)?;
        let samples =
            probe.width as usize * probe.height as usize * usize::from(probe.color.channel_count());
        let mut pixels = Pixels::zeroed(probe.color, samples);
        self.decoder.read_image(pixels.as_bytes_mut())?;
        let image = Image::from_pixels(probe.width, probe.height, probe.color, pixels)
            .with_icc_profile(profile);
        Ok((image, orientation))
    }
}

pub(crate) fn webp_animation<'r, R: BufRead + Seek + 'r>(
    reader: R,
    limits: &Limits,
) -> Result<AnimatedImage> {
    let foreign = foreign_limits(limits);
    let mut decoder = WebPDecoder::new(reader)?;
    decoder.set_limits(foreign.clone())?;
    if !decoder.has_animation() {
        let still = Foreign {
            decoder: Box::new(decoder),
            format: Format::WebP,
            limits: foreign,
        };
        let (mut image, orientation) = still.decode()?;
        image.apply_orientation(orientation);
        return Ok(AnimatedImage::from_still(image));
    }
    let (width, height) = decoder.dimensions();
    let loops = Loops::from(decoder.loop_count());
    let mut compositor = Compositor::new(width, height, limits)?;
    for frame in decoder.into_frames() {
        let frame = frame?;
        let delay = Duration::from(frame.delay());
        let (left, top) = (frame.left(), frame.top());
        let buffer = frame.into_buffer();
        let patch = Patch {
            left,
            top,
            width: buffer.width(),
            height: buffer.height(),
            rgba: buffer.as_raw(),
        };
        compositor.push(patch, Blend::Source, Dispose::Background, delay)?;
    }
    compositor.finish(loops)
}

pub(crate) fn encode_jpeg<W: Write + Seek>(
    image: &Image,
    quality: u8,
    profile: Option<&[u8]>,
    writer: W,
) -> Result<()> {
    let (width, height) = image.dimensions();
    let mut encoder = JpegEncoder::new_with_quality(writer, quality);
    if let Some(profile) = profile
        && let Err(refused) = encoder.set_icc_profile(profile.to_vec())
    {
        tracing::debug!("omitting ICC profile: {refused}");
    }
    encoder.write_image(image.as_bytes(), width, height, extended(image.color()))?;
    Ok(())
}
