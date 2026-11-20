use std::io::{Read, Seek, SeekFrom};
use std::time::Duration;

const METADATA_LIMIT: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Arrangement {
    Interleaved,
    Blocked { block: u64, lsb_first: bool },
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Layout {
    pub(crate) rate: u32,
    pub(crate) channels: u16,
    pub(crate) frames: u64,
    pub(crate) data: u64,
    pub(crate) arrangement: Arrangement,
}

impl Layout {
    pub(crate) fn bytes_per_channel(&self) -> u64 {
        self.frames.div_ceil(8)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Span {
    pub(crate) at: u64,
    pub(crate) length: Option<u64>,
}

impl Span {
    pub(crate) fn read(self, source: &mut (impl Read + Seek)) -> std::io::Result<Vec<u8>> {
        source.seek(SeekFrom::Start(self.at))?;
        let limit = self.length.unwrap_or(METADATA_LIMIT).min(METADATA_LIMIT);
        let mut bytes = Vec::new();
        source.take(limit).read_to_end(&mut bytes)?;
        Ok(bytes)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Header {
    pub(crate) rate: u32,
    pub(crate) channels: u16,
    pub(crate) frames: u64,
    pub(crate) stream: Option<(u64, Arrangement)>,
    pub(crate) id3: Option<Span>,
}

impl Header {
    pub(crate) fn layout(&self) -> Option<Layout> {
        let (data, arrangement) = self.stream?;
        Some(Layout {
            rate: self.rate,
            channels: self.channels,
            frames: self.frames,
            data,
            arrangement,
        })
    }
}

pub(crate) fn duration(frames: u64, rate: u32) -> Duration {
    Duration::from_secs_f64(frames as f64 / f64::from(rate.max(1)))
}

pub(crate) fn read_at<const N: usize>(source: &mut (impl Read + Seek), at: u64) -> Option<[u8; N]> {
    let mut bytes = [0u8; N];
    source.seek(SeekFrom::Start(at)).ok()?;
    source.read_exact(&mut bytes).ok()?;
    Some(bytes)
}
