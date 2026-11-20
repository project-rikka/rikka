use std::io::Read;

use rikka_file_format::{BinaryFormat, Codec, DecodeError, Reader, Writer};

use crate::error::Result;

const FORMAT: BinaryFormat = BinaryFormat::new("kept Spotify stream", *b"RKSPOTFY");
const TRACK_LEN: usize = 16;
const FILE_LEN: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Label {
    pub track: [u8; TRACK_LEN],
    pub file: [u8; FILE_LEN],
}

impl Label {
    pub const LEN: usize = BinaryFormat::HEADER_LEN + TRACK_LEN + FILE_LEN;

    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        FORMAT.encode(self)
    }

    pub fn read(source: &mut impl Read) -> Result<Label> {
        let mut bytes = [0; Label::LEN];
        source.read_exact(&mut bytes)?;
        Ok(FORMAT.decode(&bytes)?)
    }
}

impl Codec for Label {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        out.raw(&self.track);
        out.raw(&self.file);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Label, DecodeError> {
        Ok(Label {
            track: input.array()?,
            file: input.array()?,
        })
    }
}
