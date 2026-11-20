use std::borrow::Cow;
use std::io::{ErrorKind, Read, Seek, SeekFrom};

use crate::error::{Error, Result};
use crate::format::Format;

pub(crate) trait Stream: Read + Seek {}

impl<S: Read + Seek> Stream for S {}

enum Source<'s> {
    Slice(&'s [u8]),
    Stream(&'s mut dyn Stream),
}

pub(crate) struct Fetch<'s> {
    format: Format,
    source: Source<'s>,
    len: u64,
}

impl<'s> Fetch<'s> {
    pub(crate) fn slice(format: Format, data: &'s [u8]) -> Fetch<'s> {
        Fetch {
            format,
            source: Source::Slice(data),
            len: data.len() as u64,
        }
    }

    pub(crate) fn stream(format: Format, stream: &'s mut dyn Stream) -> Result<Fetch<'s>> {
        let len = stream.seek(SeekFrom::End(0))?;
        Ok(Fetch {
            format,
            source: Source::Stream(stream),
            len,
        })
    }

    pub(crate) fn format(&self) -> Format {
        self.format
    }

    pub(crate) fn len(&self) -> u64 {
        self.len
    }

    pub(crate) fn fetch(&mut self, at: u64, len: u64) -> Result<Cow<'_, [u8]>> {
        let short = Error::Truncated(self.format);
        let end = at.checked_add(len).ok_or(Error::Truncated(self.format))?;
        if end > self.len {
            return Err(short);
        }
        match &mut self.source {
            Source::Slice(data) => Ok(Cow::Borrowed(&data[at as usize..end as usize])),
            Source::Stream(stream) => {
                stream.seek(SeekFrom::Start(at))?;
                let mut bytes = vec![0; usize::try_from(len).map_err(|_| short)?];
                stream
                    .read_exact(&mut bytes)
                    .map_err(|error| match error.kind() {
                        ErrorKind::UnexpectedEof => Error::Truncated(self.format),
                        _ => Error::Io(error),
                    })?;
                Ok(Cow::Owned(bytes))
            }
        }
    }

    pub(crate) fn fetch_up_to(&mut self, at: u64, most: u64) -> Result<Cow<'_, [u8]>> {
        let len = most.min(self.len.saturating_sub(at));
        self.fetch(at, len)
    }
}
