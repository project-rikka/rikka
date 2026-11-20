use std::time::Duration;

use rikka_ffmpeg::Frame;

use crate::error::{Error, Result};
use crate::fetch::Fetch;
use crate::ffmpeg::decoder::Decoder;
use crate::ffmpeg::store::{Converter, Region};
use crate::format::Format;
use crate::heif::{Sequence, Track, decoding, signal};
use crate::layout::{Layout, Limits, Picture};

const NANOS_PER_SECOND: u64 = 1_000_000_000;

#[derive(Debug)]
struct TrackDecoder {
    track: Track,
    decoder: Decoder,
    next: usize,
}

impl TrackDecoder {
    fn open(format: Format, track: Track) -> Result<TrackDecoder> {
        let decoder = Decoder::open(
            format,
            decoding(track.coding.codec),
            &track.coding.config,
            (track.width, track.height),
        )?;
        Ok(TrackDecoder {
            track,
            decoder,
            next: 0,
        })
    }

    fn advance<'p>(&'p mut self, data: &[u8]) -> Result<(Option<&'p Frame>, &'p Track)> {
        let TrackDecoder {
            track,
            decoder,
            next,
        } = self;
        let samples = &track.samples;
        let mut packets = std::iter::from_fn(|| {
            let sample = samples.get(*next)?;
            let stamp = *next as i64;
            *next += 1;
            let start = sample.offset as usize;
            Some((stamp, &data[start..start + sample.size as usize]))
        });
        let frame = decoder.next(&mut packets)?;
        Ok((frame, track))
    }
}

#[derive(Debug)]
pub(crate) struct Playback<'d> {
    format: Format,
    data: &'d [u8],
    plays: Option<u32>,
    color: TrackDecoder,
    alpha: Option<TrackDecoder>,
    converter: Converter,
    canvas: Picture,
    opacity: Picture,
}

impl<'d> Playback<'d> {
    pub(crate) fn open(
        format: Format,
        data: &'d [u8],
        limits: &Limits,
    ) -> Result<Option<Playback<'d>>> {
        let mut fetch = Fetch::slice(format, data);
        let Some(sequence) = Sequence::read(&mut fetch)? else {
            return Ok(None);
        };
        let (width, height) = (sequence.color.width, sequence.color.height);
        let canvas_bytes = limits.admit(width, height, Layout::Rgba8)?;
        let opacity_bytes = limits.admit(width, height, Layout::L8)?;
        limits.admit_total(width, height, canvas_bytes + opacity_bytes)?;
        let alpha = sequence
            .alpha
            .map(|track| TrackDecoder::open(format, track))
            .transpose()?;
        Ok(Some(Playback {
            format,
            data,
            plays: sequence.plays,
            color: TrackDecoder::open(format, sequence.color)?,
            opacity: Picture::zeroed(width, height, Layout::L8),
            alpha,
            converter: Converter::new(format),
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
            format,
            data,
            color,
            alpha,
            converter,
            canvas,
            opacity,
            ..
        } = self;
        let format = *format;
        let (frame, track) = color.advance(data)?;
        let Some(frame) = frame else {
            return Ok(None);
        };
        let sample = frame
            .timestamp()
            .and_then(|stamp| usize::try_from(stamp).ok())
            .and_then(|index| track.samples.get(index))
            .ok_or_else(|| Error::malformed(format, "sequence timing"))?;
        let delay = Duration::from_nanos(
            u64::from(sample.duration) * NANOS_PER_SECOND / u64::from(track.timescale),
        );
        let region = Region::whole(canvas);
        converter.store(frame, signal(frame, track.nclx, false), canvas, region)?;
        if let Some(alpha) = alpha {
            let (frame, track) = alpha.advance(data)?;
            let frame = frame.ok_or_else(|| Error::Decoding {
                format,
                message: "missing alpha picture".to_owned(),
            })?;
            let region = Region::whole(opacity);
            converter.store(frame, signal(frame, track.nclx, true), opacity, region)?;
            let values = opacity.samples.as_bytes();
            for (pixel, &value) in canvas
                .samples
                .as_bytes_mut()
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(values)
            {
                pixel[3] = value;
            }
        }
        rgba.clear();
        rgba.extend_from_slice(canvas.samples.as_bytes());
        Ok(Some(delay))
    }
}
