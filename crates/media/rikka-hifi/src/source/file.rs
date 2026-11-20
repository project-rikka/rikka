use std::path::Path;
use std::time::Duration;

use rikka_audio_decode::{Decoder, DsdReader};

use crate::error::{Error, Result};
use crate::source::Source;

impl Source {
    pub fn file(path: impl AsRef<Path>, at: Duration) -> Result<Source> {
        let path = path.as_ref();
        match rikka_audio_decode::read(path).dsd() {
            Ok(reader) => dsd_from(reader, at).map(Source::dsd),
            Err(rikka_audio_decode::Error::NotDsd) => rikka_audio_decode::read(path)
                .decoder()
                .and_then(|decoder| decoder_from(decoder, at))
                .map(Source::decoder),
            Err(error) => Err(error),
        }
        .map_err(Error::Read)
    }
}

fn decoder_from(mut decoder: Decoder, at: Duration) -> rikka_audio_decode::Result<Decoder> {
    if !at.is_zero() {
        decoder.seek(at)?;
    }
    Ok(decoder)
}

fn dsd_from(mut reader: DsdReader, at: Duration) -> rikka_audio_decode::Result<DsdReader> {
    if !at.is_zero() {
        reader.seek(at)?;
    }
    Ok(reader)
}
