use std::path::Path;

use music_core::Codec;
use rikka_audio_decode::DsdInfo;

use crate::audio::Audio;
use crate::error::{Error, Result};

const BITS_PER_KILOBIT: u64 = 1000;

pub(crate) struct Stream {
    pub(crate) audio: Audio,
    pub(crate) id3: Option<Vec<u8>>,
}

pub(crate) fn read(path: &Path) -> Result<Stream> {
    let info = DsdInfo::read(path).map_err(|error| match error {
        rikka_audio_decode::Error::Io(error) => Error::from(error),
        other => Error::Unreadable(other.to_string()),
    })?;
    let bitrate = u64::from(info.rate) * u64::from(info.channels) / BITS_PER_KILOBIT;
    Ok(Stream {
        audio: Audio {
            codec: Codec::Dsd,
            lossless: true,
            duration: info.duration(),
            sample_rate: Some(info.rate),
            bit_depth: Some(1),
            channels: u8::try_from(info.channels).ok(),
            bitrate: u32::try_from(bitrate).ok(),
        },
        id3: info.id3,
    })
}
