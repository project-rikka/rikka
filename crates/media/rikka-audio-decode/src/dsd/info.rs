use std::time::Duration;

use rikka_types::Source;

use crate::decoding::Input;
use crate::dsd::header;
use crate::dsd::layout::duration;
use crate::error::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DsdInfo {
    pub rate: u32,
    pub channels: u16,
    pub frames: u64,
    pub compressed: bool,
    pub id3: Option<Vec<u8>>,
}

impl DsdInfo {
    pub fn read<'a>(source: impl Into<Source<'a>>) -> Result<DsdInfo> {
        let mut media = match source.into() {
            Source::File(path) => Input::file(&path, None)?.media,
            Source::Bytes(bytes) => Input::bytes(bytes.into_shared(), None).media,
        };
        let found = header(&mut media).ok_or(Error::NotDsd)?;
        let id3 = match found.id3 {
            Some(span) => Some(span.read(&mut media)?).filter(|bytes| !bytes.is_empty()),
            None => None,
        };
        Ok(DsdInfo {
            rate: found.rate,
            channels: found.channels,
            frames: found.frames,
            compressed: found.stream.is_none(),
            id3,
        })
    }

    #[must_use]
    pub fn duration(&self) -> Duration {
        duration(self.frames, self.rate)
    }
}
