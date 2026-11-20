use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

use crate::error::{Error, Result};

const PCM: u16 = 1;
const EXTENSIBLE: u16 = 0xFFFE;
const PCM_SUBFORMAT: [u8; 16] = [
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
];
const PLAIN_FORMAT: u32 = 16;
const EXTENSIBLE_FORMAT: u32 = 40;
const EXTENSION_SIZE: u16 = 22;
const RIFF_SIZE_AT: u64 = 4;
const LIMIT: u64 = u32::MAX as u64;
const SPEAKERS: u16 = 18;
const WRITE_BUFFER: usize = 1 << 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WavDepth {
    Pcm16,
    Pcm24,
}

impl WavDepth {
    fn bytes(self) -> u16 {
        match self {
            WavDepth::Pcm16 => 2,
            WavDepth::Pcm24 => 3,
        }
    }
}

pub struct WavWriter {
    file: BufWriter<File>,
    channels: u16,
    depth: WavDepth,
    header: u64,
    data: u64,
    dither: Dither,
    encoded: Vec<u8>,
}

impl WavWriter {
    pub fn create(
        path: impl AsRef<Path>,
        sample_rate: u32,
        channels: u16,
        depth: WavDepth,
    ) -> Result<WavWriter> {
        if channels == 0 || channels > SPEAKERS {
            return Err(Error::WavChannels(channels));
        }
        let mut file = BufWriter::with_capacity(WRITE_BUFFER, File::create(path)?);
        let header = header(&mut file, sample_rate, channels, depth)?;
        Ok(WavWriter {
            file,
            channels,
            depth,
            header,
            data: 0,
            dither: Dither::default(),
            encoded: Vec::new(),
        })
    }

    pub fn write(&mut self, samples: &[f32]) -> Result<()> {
        if !samples.len().is_multiple_of(usize::from(self.channels)) {
            return Err(Error::PartialFrame {
                samples: samples.len(),
                channels: self.channels,
            });
        }
        let bytes = samples.len() as u64 * u64::from(self.depth.bytes());
        if self.header + self.data + bytes + 1 > LIMIT {
            return Err(Error::WavTooLarge);
        }
        self.encoded.clear();
        match self.depth {
            WavDepth::Pcm16 => {
                for sample in samples {
                    let value = f64::from(sample.clamp(-1.0, 1.0)) * 32768.0 + self.dither.next();
                    let value = value.round().clamp(-32768.0, 32767.0) as i16;
                    self.encoded.extend_from_slice(&value.to_le_bytes());
                }
            }
            WavDepth::Pcm24 => {
                for sample in samples {
                    let value = f64::from(sample.clamp(-1.0, 1.0)) * 8_388_608.0;
                    let value = value.round().clamp(-8_388_608.0, 8_388_607.0) as i32;
                    self.encoded.extend_from_slice(&value.to_le_bytes()[..3]);
                }
            }
        }
        self.file.write_all(&self.encoded)?;
        self.data += bytes;
        Ok(())
    }

    pub fn finish(mut self) -> Result<()> {
        if !self.data.is_multiple_of(2) {
            self.file.write_all(&[0])?;
        }
        let padded = self.data + self.data % 2;
        let riff = self.header - 8 + padded;
        self.file.seek(SeekFrom::Start(RIFF_SIZE_AT))?;
        self.file.write_all(&(riff as u32).to_le_bytes())?;
        self.file.seek(SeekFrom::Start(self.header - 4))?;
        self.file.write_all(&(self.data as u32).to_le_bytes())?;
        let file = self
            .file
            .into_inner()
            .map_err(|failed| Error::Io(failed.into_error()))?;
        file.sync_all()?;
        Ok(())
    }
}

impl std::fmt::Debug for WavWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WavWriter")
            .field("channels", &self.channels)
            .field("depth", &self.depth)
            .field("data", &self.data)
            .finish_non_exhaustive()
    }
}

fn header(file: &mut impl Write, sample_rate: u32, channels: u16, depth: WavDepth) -> Result<u64> {
    let bytes = depth.bytes();
    let bits = bytes * 8;
    let block = channels * bytes;
    let extensible = depth != WavDepth::Pcm16 || channels > 2;
    let format = match extensible {
        true => EXTENSIBLE_FORMAT,
        false => PLAIN_FORMAT,
    };
    let mut head = Vec::with_capacity(68);
    head.extend_from_slice(b"RIFF");
    head.extend_from_slice(&0u32.to_le_bytes());
    head.extend_from_slice(b"WAVE");
    head.extend_from_slice(b"fmt ");
    head.extend_from_slice(&format.to_le_bytes());
    head.extend_from_slice(&(if extensible { EXTENSIBLE } else { PCM }).to_le_bytes());
    head.extend_from_slice(&channels.to_le_bytes());
    head.extend_from_slice(&sample_rate.to_le_bytes());
    head.extend_from_slice(&(sample_rate * u32::from(block)).to_le_bytes());
    head.extend_from_slice(&block.to_le_bytes());
    head.extend_from_slice(&bits.to_le_bytes());
    if extensible {
        head.extend_from_slice(&EXTENSION_SIZE.to_le_bytes());
        head.extend_from_slice(&bits.to_le_bytes());
        head.extend_from_slice(&speakers(channels).to_le_bytes());
        head.extend_from_slice(&PCM_SUBFORMAT);
    }
    head.extend_from_slice(b"data");
    head.extend_from_slice(&0u32.to_le_bytes());
    file.write_all(&head)?;
    Ok(head.len() as u64)
}

fn speakers(channels: u16) -> u32 {
    match channels {
        1 => 0x4,
        2 => 0x3,
        6 => 0x3F,
        8 => 0x63F,
        channels => (1u32 << channels) - 1,
    }
}

#[derive(Debug)]
struct Dither {
    state: u64,
}

impl Default for Dither {
    fn default() -> Dither {
        Dither {
            state: 0x9E37_79B9_7F4A_7C15,
        }
    }
}

impl Dither {
    fn next(&mut self) -> f64 {
        self.uniform() - self.uniform()
    }

    fn uniform(&mut self) -> f64 {
        self.state ^= self.state >> 12;
        self.state ^= self.state << 25;
        self.state ^= self.state >> 27;
        let value = self.state.wrapping_mul(0x2545_F491_4F6C_DD1D);
        (value >> 11) as f64 / (1u64 << 53) as f64
    }
}
