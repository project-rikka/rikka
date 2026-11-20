use std::io::{Read, Seek};
use std::sync::Arc;

use rikka_image_ffmpeg::{Animation, Layout, Samples, Settings};

use crate::animated::{AnimatedImage, Blend, Compositor, Dispose, Patch};
use crate::codecs::{Limits, Probe};
use crate::color_type::ColorType;
use crate::error::{Error, Result};
use crate::format::Format;
use crate::loops::Loops;
use crate::orientation::Orientation;
use crate::pixels::Pixels;
use crate::raster::Image;

fn backend(format: Format) -> rikka_image_ffmpeg::Format {
    match format {
        Format::Avif => rikka_image_ffmpeg::Format::Avif,
        Format::Heif => rikka_image_ffmpeg::Format::Heif,
        Format::Tiff => rikka_image_ffmpeg::Format::Tiff,
        Format::OpenExr => rikka_image_ffmpeg::Format::OpenExr,
        Format::Jpeg2000 => rikka_image_ffmpeg::Format::Jpeg2000,
        Format::JpegXl => rikka_image_ffmpeg::Format::JpegXl,
        other => unreachable!("{other} is not coded through FFmpeg"),
    }
}

fn error(format: Format, error: rikka_image_ffmpeg::Error) -> Error {
    use rikka_image_ffmpeg::Error as Backend;

    match error {
        Backend::Io(io) => Error::Io(io),
        Backend::Unavailable(_) => Error::NeedsFfmpeg(format),
        Backend::NoDecoder(codec) => Error::NeedsDecoder { format, codec },
        Backend::NoEncoder(codec) => Error::NeedsEncoder { format, codec },
        Backend::InvalidSize { width, height } => Error::InvalidSize { width, height },
        over @ Backend::LimitsExceeded { .. } => Error::LimitsExceeded(format!("{format}: {over}")),
        refused @ Backend::Unsupported { .. } => Error::Unsupported(refused.to_string()),
        refused @ (Backend::Encoding { .. } | Backend::PixelDataSize { .. }) => {
            Error::Encoding(refused.to_string())
        }
        damaged => Error::Decoding(damaged.to_string()),
    }
}

const fn color_of(layout: Layout) -> ColorType {
    match layout {
        Layout::L8 => ColorType::L8,
        Layout::La8 => ColorType::La8,
        Layout::Rgb8 => ColorType::Rgb8,
        Layout::Rgba8 => ColorType::Rgba8,
        Layout::L16 => ColorType::L16,
        Layout::La16 => ColorType::La16,
        Layout::Rgb16 => ColorType::Rgb16,
        Layout::Rgba16 => ColorType::Rgba16,
        Layout::Rgb32F => ColorType::Rgb32F,
        Layout::Rgba32F => ColorType::Rgba32F,
    }
}

const fn layout_of(color: ColorType) -> Layout {
    match color {
        ColorType::L8 => Layout::L8,
        ColorType::La8 => Layout::La8,
        ColorType::Rgb8 => Layout::Rgb8,
        ColorType::Rgba8 => Layout::Rgba8,
        ColorType::L16 => Layout::L16,
        ColorType::La16 => Layout::La16,
        ColorType::Rgb16 => Layout::Rgb16,
        ColorType::Rgba16 => Layout::Rgba16,
        ColorType::Rgb32F => Layout::Rgb32F,
        ColorType::Rgba32F => Layout::Rgba32F,
    }
}

const fn orientation_of(orientation: rikka_image_ffmpeg::Orientation) -> Orientation {
    use rikka_image_ffmpeg::Orientation as Backend;

    match orientation {
        Backend::Upright => Orientation::Upright,
        Backend::Rotate90 => Orientation::Rotate90,
        Backend::Rotate180 => Orientation::Rotate180,
        Backend::Rotate270 => Orientation::Rotate270,
        Backend::FlipHorizontal => Orientation::FlipHorizontal,
        Backend::FlipVertical => Orientation::FlipVertical,
        Backend::Rotate90FlipHorizontal => Orientation::Rotate90FlipHorizontal,
        Backend::Rotate270FlipHorizontal => Orientation::Rotate270FlipHorizontal,
    }
}

fn limits_of(limits: &Limits) -> rikka_image_ffmpeg::Limits {
    let (max_width, max_height) = limits.max_dimensions.unwrap_or((u32::MAX, u32::MAX));
    rikka_image_ffmpeg::Limits {
        max_width,
        max_height,
        max_bytes: limits.max_alloc.unwrap_or(u64::MAX),
    }
}

pub(crate) fn probe<R: Read + Seek>(format: Format, reader: R) -> Result<(Probe, Orientation)> {
    let info =
        rikka_image_ffmpeg::probe(backend(format), reader).map_err(|cause| error(format, cause))?;
    Ok((
        Probe {
            width: info.width,
            height: info.height,
            color: color_of(info.layout),
        },
        orientation_of(info.orientation),
    ))
}

pub(crate) fn decode(format: Format, data: &[u8], limits: &Limits) -> Result<(Image, Orientation)> {
    let decoded = rikka_image_ffmpeg::decode(backend(format), data, &limits_of(limits))
        .map_err(|cause| error(format, cause))?;
    let picture = decoded.picture;
    let pixels = match picture.samples {
        Samples::U8(samples) => Pixels::U8(samples),
        Samples::U16(samples) => Pixels::U16(samples),
        Samples::F32(samples) => Pixels::F32(samples),
    };
    let image = Image::from_pixels(
        picture.width,
        picture.height,
        color_of(picture.layout),
        pixels,
    )
    .with_icc_profile(decoded.icc_profile.map(Arc::from));
    Ok((image, orientation_of(decoded.orientation)))
}

pub(crate) fn animation(
    format: Format,
    data: &[u8],
    limits: &Limits,
) -> Result<Option<AnimatedImage>> {
    let failed = |cause| error(format, cause);
    let Some(mut animation) =
        Animation::open(backend(format), data, &limits_of(limits)).map_err(failed)?
    else {
        return Ok(None);
    };
    let (width, height) = (animation.width(), animation.height());
    let mut compositor = Compositor::new(width, height, limits)?;
    let mut rgba = Vec::new();
    while let Some(delay) = animation.next_frame(&mut rgba).map_err(failed)? {
        let patch = Patch {
            left: 0,
            top: 0,
            width,
            height,
            rgba: &rgba,
        };
        compositor.push(patch, Blend::Source, Dispose::Keep, delay)?;
    }
    let loops = match animation.plays() {
        Some(plays) => Loops::Finite(plays),
        None => Loops::Infinite,
    };
    compositor.finish(loops).map(Some)
}

pub(crate) fn encode(
    format: Format,
    image: &Image,
    quality: u8,
    speed: u8,
    profile: Option<&[u8]>,
) -> Result<Vec<u8>> {
    let pixels = rikka_image_ffmpeg::Pixels {
        width: image.width(),
        height: image.height(),
        layout: layout_of(image.color()),
        bytes: image.as_bytes(),
    };
    rikka_image_ffmpeg::encode(
        backend(format),
        pixels,
        Settings {
            quality,
            speed,
            profile,
        },
    )
    .map_err(|cause| error(format, cause))
}
