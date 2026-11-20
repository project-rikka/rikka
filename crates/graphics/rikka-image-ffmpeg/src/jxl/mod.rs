mod bits;
mod color;
mod container;
mod header;
mod playback;

use rikka_ffmpeg::Frame;
use rikka_ffmpeg::avutil::AVRational;

use crate::decode::Decoded;
use crate::error::{Error, Result};
use crate::fetch::Fetch;
use crate::ffmpeg::decoder::{Decoder, Decoding};
use crate::ffmpeg::store::{Converter, Region, Signal};
use crate::format::Format;
use crate::layout::{Limits, Picture};
use crate::orientation::Orientation;
use crate::probe::Header;

pub(crate) use playback::Playback;

use header::{Alpha, ImageHeader};

const FORMAT: Format = Format::JpegXl;

pub(crate) fn header(fetch: &mut Fetch<'_>) -> Result<Header> {
    let image = container::image_header(fetch)?;
    Ok(Header {
        width: image.width,
        height: image.height,
        layout: image.layout(),
        orientation: image.orientation,
        profile: None,
        premultiplied: image.alpha == Some(Alpha::Premultiplied),
    })
}

pub(crate) fn decode(data: &[u8], limits: &Limits) -> Result<Decoded> {
    let image = container::image_header(&mut Fetch::slice(FORMAT, data))?;
    let layout = image.layout();
    let (width, height) = image.displayed();
    limits.admit(width, height, layout)?;
    let mut decoder = open(&image)?;
    let frame = decoder
        .next(&mut std::iter::once((0, data)))?
        .ok_or_else(|| Error::Decoding {
            format: FORMAT,
            message: "no picture".to_owned(),
        })?;
    let mut picture = Picture::zeroed(width, height, layout);
    store(&image, frame, &mut Converter::new(FORMAT), &mut picture)?;
    Ok(Decoded {
        picture,
        icc_profile: color::profile(frame, image.gray)?,
        orientation: Orientation::Upright,
    })
}

fn open(image: &ImageHeader) -> Result<Decoder> {
    let size = image.displayed();
    match image.timing {
        Some(timing) => {
            let seconds_per_tick = AVRational {
                num: timing.ticks.denominator as i32,
                den: timing.ticks.numerator as i32,
            };
            Decoder::open_timed(FORMAT, Decoding::JpegXl, size, seconds_per_tick)
        }
        None => Decoder::open(FORMAT, Decoding::JpegXl, &[], size),
    }
}

fn store(
    image: &ImageHeader,
    frame: &Frame,
    converter: &mut Converter,
    target: &mut Picture,
) -> Result<()> {
    let decoded = (
        u32::try_from(frame.width()).ok(),
        u32::try_from(frame.height()).ok(),
    );
    if decoded != (Some(target.width), Some(target.height)) {
        return Err(Error::Decoding {
            format: FORMAT,
            message: "picture size differs from the header".to_owned(),
        });
    }
    let region = Region::whole(target);
    converter.store(frame, Signal::of(frame, true), target, region)?;
    if image.alpha == Some(Alpha::Premultiplied) {
        target.unpremultiply();
    }
    Ok(())
}
