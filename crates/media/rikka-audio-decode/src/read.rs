use std::path::Path;
use std::time::Duration;

use rikka_types::Source;

use crate::decoder::Decoder;
use crate::decoding::{self, Input, Pipeline};
use crate::dsd::DsdReader;
use crate::encoding::Encoding;
use crate::error::Result;
use crate::format::AudioFormat;
use crate::sound::Sound;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AudioInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub duration: Option<Duration>,
    pub frames: Option<u64>,
    pub bits_per_sample: Option<u32>,
    pub encoding: Encoding,
}

#[must_use]
pub fn read<'a>(source: impl Into<Source<'a>>) -> Reading<'a> {
    Reading {
        source: source.into(),
        format: None,
    }
}

#[derive(Clone, Debug)]
pub struct Reading<'a> {
    source: Source<'a>,
    format: Option<AudioFormat>,
}

impl<'a> Reading<'a> {
    #[must_use]
    pub fn format(mut self, format: AudioFormat) -> Reading<'a> {
        self.format = Some(format);
        self
    }

    pub fn decode(self) -> Result<Sound> {
        decoding::decode_all(self.input()?)
    }

    pub fn decoder(self) -> Result<Decoder> {
        Ok(Decoder::new(Pipeline::open(self.input()?)?))
    }

    pub fn probe(self) -> Result<AudioInfo> {
        Ok(*Pipeline::open(self.input()?)?.info())
    }

    pub fn dsd(self) -> Result<DsdReader> {
        DsdReader::open(self.input()?.media)
    }

    fn input(self) -> Result<Input> {
        Ok(match self.source {
            Source::File(path) => Input::file(&path, self.format)?,
            Source::Bytes(bytes) => Input::bytes(bytes.into_shared(), self.format),
        })
    }
}

impl Sound {
    pub fn open(path: impl AsRef<Path>) -> Result<Sound> {
        read(path.as_ref()).decode()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Sound> {
        read(bytes).decode()
    }
}
