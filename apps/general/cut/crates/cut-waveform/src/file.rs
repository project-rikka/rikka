use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rikka_file_format::{BinaryFormat, DecodeError, Reader, Writer};

use crate::error::{Error, Result};
use crate::peaks::{BASE, Bucket, Peaks};

const FORMAT: BinaryFormat = BinaryFormat::new("waveform cache", *b"RKPEAKS\0");
const BUCKET_BYTES: usize = 6;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Fingerprint {
    path: PathBuf,
    length: u64,
    modified: u128,
}

impl Fingerprint {
    pub(crate) fn read(path: &Path) -> Result<Fingerprint> {
        let metadata = std::fs::metadata(path).map_err(|error| Error::Read {
            path: path.to_path_buf(),
            error,
        })?;
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |since| since.as_nanos());
        Ok(Fingerprint {
            path: path.to_path_buf(),
            length: metadata.len(),
            modified,
        })
    }

    #[must_use]
    pub(crate) fn cache_name(&self) -> String {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in self.path.as_os_str().as_encoded_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        format!("{hash:016x}.peaks")
    }
}

#[must_use]
pub(crate) fn encode(fingerprint: &Fingerprint, peaks: &Peaks) -> Vec<u8> {
    let path = fingerprint.path.as_os_str().as_encoded_bytes();
    let base = peaks.base();
    let mut bytes =
        Vec::with_capacity(BinaryFormat::HEADER_LEN + 64 + path.len() + base.len() * BUCKET_BYTES);
    let mut out = Writer::new(&mut bytes);
    FORMAT.write_header(&mut out);
    out.bytes(path);
    out.put(&fingerprint.length);
    out.put(&fingerprint.modified);
    out.put(&peaks.rate());
    out.put(&peaks.channels());
    out.put(&peaks.frames());
    out.put(&(BASE as u32));
    out.count64(base.len());
    for bucket in base {
        out.put(&bucket.low);
        out.put(&bucket.high);
        out.put(&bucket.rms);
    }
    bytes
}

pub(crate) fn decode(fingerprint: &Fingerprint, bytes: &[u8]) -> Result<Peaks> {
    let mut input = Reader::new(bytes);
    FORMAT.read_header(&mut input)?;
    let path = input.bytes()?;
    let length: u64 = input.get()?;
    let modified: u128 = input.get()?;
    if path != fingerprint.path.as_os_str().as_encoded_bytes()
        || length != fingerprint.length
        || modified != fingerprint.modified
    {
        return Err(Error::Stale);
    }
    let rate: u32 = input.get()?;
    let channels: u16 = input.get()?;
    let frames: u64 = input.get()?;
    let base = u64::from(input.get::<u32>()?);
    let count = input.count64(BUCKET_BYTES)?;
    if base != BASE
        || channels == 0
        || rate == 0
        || frames.div_ceil(BASE).checked_mul(u64::from(channels)) != Some(count as u64)
    {
        return Err(Error::Malformed);
    }
    let buckets = (0..count)
        .map(|_| {
            Ok(Bucket {
                low: input.get()?,
                high: input.get()?,
                rms: input.get()?,
            })
        })
        .collect::<Result<Vec<_>, DecodeError>>()?;
    input.finish()?;
    Ok(Peaks::from_base(rate, channels, frames, buckets))
}
