use rikka_file_format::{BinaryFormat, Codec, Reader, Writer};

use super::{Entry, Recording};
use crate::{Error, Result};

pub(super) const FORMAT: BinaryFormat = BinaryFormat::new("physics recording", *b"PHYSREC\0");

impl Codec for Entry {
    type Error = Error;

    fn encode(&self, out: &mut Writer<'_>) {
        match self {
            Entry::Setup(setup) => {
                out.tag(0);
                out.put(setup);
            }
            Entry::Call(call) => {
                out.tag(1);
                out.put(call);
            }
            Entry::Tick(hash) => {
                out.tag(2);
                out.put(hash);
            }
        }
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self> {
        match input.tag()? {
            0 => Ok(Entry::Setup(input.get()?)),
            1 => Ok(Entry::Call(input.get()?)),
            2 => Ok(Entry::Tick(input.get()?)),
            _ => Err(Error::MalformedRecording("unknown entry")),
        }
    }
}

impl Codec for Recording {
    type Error = Error;

    fn encode(&self, out: &mut Writer<'_>) {
        out.put(&self.settings);
        out.put(&self.entries);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self> {
        Ok(Recording {
            settings: input.get()?,
            entries: input.get()?,
        })
    }
}
