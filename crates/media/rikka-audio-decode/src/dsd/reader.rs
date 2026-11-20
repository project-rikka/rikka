use std::io::{Read, Seek, SeekFrom};
use std::time::Duration;

use crate::decoding::Media;
use crate::dsd::header;
use crate::dsd::layout::{Arrangement, Layout, duration};
use crate::error::{Error, Result};

const CHUNK: u64 = 4096;

pub struct DsdReader {
    source: Box<dyn Media>,
    layout: Layout,
    position: u64,
    gathered: Vec<u8>,
}

impl DsdReader {
    pub(crate) fn open(mut source: Box<dyn Media>) -> Result<DsdReader> {
        let layout = header(&mut source)
            .and_then(|found| found.layout())
            .ok_or(Error::NotDsd)?;
        Ok(DsdReader {
            source,
            layout,
            position: 0,
            gathered: Vec::new(),
        })
    }

    #[must_use]
    pub fn rate(&self) -> u32 {
        self.layout.rate
    }

    #[must_use]
    pub fn channels(&self) -> u16 {
        self.layout.channels
    }

    #[must_use]
    pub fn frames(&self) -> u64 {
        self.layout.frames
    }

    #[must_use]
    pub fn duration(&self) -> Duration {
        duration(self.layout.frames, self.layout.rate)
    }

    pub fn read(&mut self, out: &mut Vec<u8>) -> Result<bool> {
        let total = self.layout.bytes_per_channel();
        if self.position >= total {
            return Ok(false);
        }
        let channels = u64::from(self.layout.channels);
        match self.layout.arrangement {
            Arrangement::Interleaved => {
                let count = CHUNK.min(total - self.position);
                let at = self.layout.data + self.position * channels;
                let start = out.len();
                out.resize(start + (count * channels) as usize, 0);
                self.source.seek(SeekFrom::Start(at))?;
                self.source.read_exact(&mut out[start..])?;
                self.position += count;
            }
            Arrangement::Blocked { block, lsb_first } => {
                let within = self.position % block;
                let count = (block - within).min(total - self.position);
                let base = self.layout.data + (self.position - within) * channels + within;
                self.gathered.resize((count * channels) as usize, 0);
                for (channel, run) in self.gathered.chunks_exact_mut(count as usize).enumerate() {
                    self.source
                        .seek(SeekFrom::Start(base + channel as u64 * block))?;
                    self.source.read_exact(run)?;
                }
                out.reserve((count * channels) as usize);
                for index in 0..count as usize {
                    for channel in 0..channels as usize {
                        let byte = self.gathered[channel * count as usize + index];
                        out.push(if lsb_first { byte.reverse_bits() } else { byte });
                    }
                }
                self.position += count;
            }
        }
        Ok(true)
    }

    pub fn seek(&mut self, to: Duration) -> Result<Duration> {
        let byte = (to.as_secs_f64() * f64::from(self.layout.rate) / 8.0).floor() as u64;
        self.position = byte.min(self.layout.bytes_per_channel());
        Ok(Duration::from_secs_f64(
            (self.position * 8) as f64 / f64::from(self.layout.rate),
        ))
    }
}

impl std::fmt::Debug for DsdReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DsdReader")
            .field("rate", &self.layout.rate)
            .field("channels", &self.layout.channels)
            .field("frames", &self.layout.frames)
            .finish_non_exhaustive()
    }
}
