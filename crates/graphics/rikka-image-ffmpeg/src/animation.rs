use std::time::Duration;

use crate::error::Result;
use crate::format::Format;
use crate::layout::Limits;
use crate::{heif, jxl};

#[derive(Debug)]
pub struct Animation<'d> {
    source: Source<'d>,
}

#[derive(Debug)]
enum Source<'d> {
    Sequence(Box<heif::Playback<'d>>),
    JpegXl(jxl::Playback<'d>),
}

impl<'d> Animation<'d> {
    pub fn open(format: Format, data: &'d [u8], limits: &Limits) -> Result<Option<Animation<'d>>> {
        let source = match format {
            Format::Avif | Format::Heif => heif::Playback::open(format, data, limits)?
                .map(|playback| Source::Sequence(Box::new(playback))),
            Format::JpegXl => jxl::Playback::open(data, limits)?.map(Source::JpegXl),
            Format::Tiff | Format::OpenExr | Format::Jpeg2000 => None,
        };
        Ok(source.map(|source| Animation { source }))
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        match &self.source {
            Source::Sequence(playback) => playback.width(),
            Source::JpegXl(playback) => playback.width(),
        }
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        match &self.source {
            Source::Sequence(playback) => playback.height(),
            Source::JpegXl(playback) => playback.height(),
        }
    }

    #[must_use]
    pub fn plays(&self) -> Option<u32> {
        match &self.source {
            Source::Sequence(playback) => playback.plays(),
            Source::JpegXl(playback) => playback.plays(),
        }
    }

    pub fn next_frame(&mut self, rgba: &mut Vec<u8>) -> Result<Option<Duration>> {
        match &mut self.source {
            Source::Sequence(playback) => playback.next_frame(rgba),
            Source::JpegXl(playback) => playback.next_frame(rgba),
        }
    }
}
