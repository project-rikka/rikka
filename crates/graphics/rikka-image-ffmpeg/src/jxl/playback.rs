use std::time::Duration;

use rikka_ffmpeg::Frame;

use crate::error::{Error, Result};
use crate::fetch::Fetch;
use crate::ffmpeg::decoder::Decoder;
use crate::ffmpeg::store::Converter;
use crate::jxl::header::{ImageHeader, Ticks};
use crate::jxl::{FORMAT, container, open, store};
use crate::layout::{Layout, Limits, Picture};

const NANOS_PER_SECOND: u128 = 1_000_000_000;

#[derive(Debug)]
pub(crate) struct Playback<'d> {
    data: &'d [u8],
    sent: bool,
    image: ImageHeader,
    ticks: Ticks,
    plays: Option<u32>,
    decoder: Decoder,
    converter: Converter,
    canvas: Picture,
}

impl<'d> Playback<'d> {
    pub(crate) fn open(data: &'d [u8], limits: &Limits) -> Result<Option<Playback<'d>>> {
        let image = container::image_header(&mut Fetch::slice(FORMAT, data))?;
        let Some(timing) = image.timing else {
            return Ok(None);
        };
        let (width, height) = image.displayed();
        limits.admit(width, height, Layout::Rgba8)?;
        Ok(Some(Playback {
            data,
            sent: false,
            decoder: open(&image)?,
            image,
            ticks: timing.ticks,
            plays: timing.plays,
            converter: Converter::new(FORMAT),
            canvas: Picture::zeroed(width, height, Layout::Rgba8),
        }))
    }

    pub(crate) fn width(&self) -> u32 {
        self.canvas.width
    }

    pub(crate) fn height(&self) -> u32 {
        self.canvas.height
    }

    pub(crate) fn plays(&self) -> Option<u32> {
        self.plays
    }

    pub(crate) fn next_frame(&mut self, rgba: &mut Vec<u8>) -> Result<Option<Duration>> {
        let Playback {
            data,
            sent,
            image,
            ticks,
            decoder,
            converter,
            canvas,
            ..
        } = self;
        let mut packets = std::iter::from_fn(|| match *sent {
            true => None,
            false => {
                *sent = true;
                Some((0, *data))
            }
        });
        let Some(frame) = decoder.next(&mut packets)? else {
            return Ok(None);
        };
        let delay = delay(frame, *ticks)?;
        store(image, frame, converter, canvas)?;
        rgba.clear();
        rgba.extend_from_slice(canvas.samples.as_bytes());
        Ok(Some(delay))
    }
}

fn delay(frame: &Frame, ticks: Ticks) -> Result<Duration> {
    let timing = || Error::malformed(FORMAT, "frame duration");
    let span = u32::try_from(frame.span()).map_err(|_| timing())?;
    let nanos = u128::from(span) * u128::from(ticks.denominator) * NANOS_PER_SECOND
        / u128::from(ticks.numerator);
    u64::try_from(nanos)
        .map(Duration::from_nanos)
        .map_err(|_| timing())
}
