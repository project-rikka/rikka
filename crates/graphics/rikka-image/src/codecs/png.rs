use std::io::{Read, Seek};
use std::sync::Arc;
use std::time::Duration;

use rikka_png::{Decoder, Encoder, Layout, Level, Metadata, Samples, Text};

use crate::animated::{Blend, Compositor, Dispose, Patch};
use crate::codecs::{Limits, Probe};
use crate::color_type::ColorType;
use crate::error::{Error, Result};
use crate::loops::Loops;
use crate::orientation::Orientation;
use crate::pixels::Pixels;
use crate::raster::Image;
use crate::write::Compression;

const DEFAULT_DELAY_DENOMINATOR: u64 = 100;

fn color_of(layout: Layout) -> ColorType {
    match layout {
        Layout::L8 => ColorType::L8,
        Layout::La8 => ColorType::La8,
        Layout::Rgb8 => ColorType::Rgb8,
        Layout::Rgba8 => ColorType::Rgba8,
        Layout::L16 => ColorType::L16,
        Layout::La16 => ColorType::La16,
        Layout::Rgb16 => ColorType::Rgb16,
        Layout::Rgba16 => ColorType::Rgba16,
    }
}

fn layout_of(color: ColorType) -> Layout {
    match color {
        ColorType::L8 => Layout::L8,
        ColorType::La8 => Layout::La8,
        ColorType::Rgb8 => Layout::Rgb8,
        ColorType::Rgba8 => Layout::Rgba8,
        ColorType::L16 => Layout::L16,
        ColorType::La16 => Layout::La16,
        ColorType::Rgb16 | ColorType::Rgb32F => Layout::Rgb16,
        ColorType::Rgba16 | ColorType::Rgba32F => Layout::Rgba16,
    }
}

fn limits_of(limits: &Limits) -> rikka_png::Limits {
    let (max_width, max_height) = limits.max_dimensions.unwrap_or((u32::MAX, u32::MAX));
    rikka_png::Limits {
        max_width,
        max_height,
        max_bytes: limits.max_alloc.unwrap_or(u64::MAX),
    }
}

impl From<rikka_png::Error> for Error {
    fn from(error: rikka_png::Error) -> Error {
        use rikka_png::Error as Png;

        match error {
            Png::Io(io) => Error::Io(io),
            over @ Png::LimitsExceeded { .. } => Error::LimitsExceeded(format!("PNG: {over}")),
            Png::InvalidSize { width, height } => Error::InvalidSize { width, height },
            refused @ (Png::PixelDataSize { .. } | Png::TooLarge) => {
                Error::Encoding(format!("PNG: {refused}"))
            }
            damaged => Error::Decoding(format!("PNG: {damaged}")),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Described {
    pub(crate) probe: Probe,
    pub(crate) orientation: Orientation,
    pub(crate) text: Vec<(String, String)>,
}

pub(crate) fn probe<R: Read + Seek>(reader: R) -> Result<Described> {
    let info = rikka_png::probe(reader)?;
    Ok(Described {
        probe: Probe {
            width: info.width,
            height: info.height,
            color: color_of(info.layout),
        },
        orientation: orientation(info.exif.as_deref()),
        text: info
            .text
            .into_iter()
            .map(|entry| (entry.keyword, entry.text))
            .collect(),
    })
}

fn orientation(exif: Option<&[u8]>) -> Orientation {
    exif.and_then(Orientation::from_exif).unwrap_or_default()
}

pub(crate) fn decode(data: &[u8], limits: &Limits) -> Result<(Image, Orientation)> {
    let decoded = Decoder::new().decode(data, &limits_of(limits))?;
    let picture = decoded.picture;
    let pixels = match picture.samples {
        Samples::U8(samples) => Pixels::U8(samples),
        Samples::U16(samples) => Pixels::U16(samples),
    };
    let image = Image::from_pixels(
        picture.width,
        picture.height,
        color_of(picture.layout),
        pixels,
    )
    .with_icc_profile(decoded.icc_profile.map(Arc::from));
    Ok((image, orientation(decoded.exif.as_deref())))
}

pub(crate) fn animation(data: &[u8], limits: &Limits) -> Result<Option<(Compositor, Loops)>> {
    let Some(animation) = Decoder::new().animation(data, &limits_of(limits))? else {
        return Ok(None);
    };
    let mut compositor = Compositor::new(animation.width, animation.height, limits)?;
    for frame in &animation.frames {
        let control = frame.control;
        let denominator = match control.delay_denominator {
            0 => DEFAULT_DELAY_DENOMINATOR,
            denominator => u64::from(denominator),
        };
        let delay =
            Duration::from_micros(u64::from(control.delay_numerator) * 1_000_000 / denominator);
        let patch = Patch {
            left: control.left,
            top: control.top,
            width: control.width,
            height: control.height,
            rgba: &frame.rgba,
        };
        let blend = match control.blend {
            rikka_png::Blend::Source => Blend::Source,
            rikka_png::Blend::Over => Blend::Over,
        };
        let dispose = match control.dispose {
            rikka_png::Dispose::Keep => Dispose::Keep,
            rikka_png::Dispose::Background => Dispose::Background,
            rikka_png::Dispose::Previous => Dispose::Previous,
        };
        compositor.push(patch, blend, dispose, delay)?;
    }
    let loops = match animation.plays {
        0 => Loops::Infinite,
        plays => Loops::Finite(plays),
    };
    Ok(Some((compositor, loops)))
}

pub(crate) fn encode(
    image: &Image,
    compression: Compression,
    profile: Option<&[u8]>,
    text: &[(String, String)],
    out: &mut Vec<u8>,
) -> Result<()> {
    let level = match compression {
        Compression::Fast => Level::Fast,
        Compression::Balanced => Level::Balanced,
        Compression::Best => Level::Best,
    };
    let text: Vec<Text> = text
        .iter()
        .map(|(keyword, text)| Text {
            keyword: keyword.clone(),
            text: text.clone(),
        })
        .collect();
    Encoder::new(level).encode(
        rikka_png::Pixels {
            width: image.width(),
            height: image.height(),
            layout: layout_of(image.color()),
            bytes: image.as_bytes(),
        },
        &Metadata {
            icc_profile: profile,
            text: &text,
        },
        out,
    )?;
    Ok(())
}
