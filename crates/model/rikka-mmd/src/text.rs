use rikka_model_core::{Error, Result};

use rikka_model_core::{ByteReader, ByteWriter};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PmxEncoding {
    #[default]
    Utf16,
    Utf8,
}

impl PmxEncoding {
    pub(crate) fn from_byte(byte: u8) -> Result<Self> {
        match byte {
            0 => Ok(Self::Utf16),
            1 => Ok(Self::Utf8),
            other => Err(Error::Damaged(format!("unknown PMX text encoding {other}"))),
        }
    }

    pub(crate) fn byte(self) -> u8 {
        match self {
            Self::Utf16 => 0,
            Self::Utf8 => 1,
        }
    }

    pub(crate) fn decode(self, bytes: &[u8]) -> String {
        match self {
            Self::Utf16 => decode_utf16(bytes),
            Self::Utf8 => String::from_utf8_lossy(bytes).into_owned(),
        }
    }

    pub(crate) fn encode(self, text: &str) -> Vec<u8> {
        match self {
            Self::Utf16 => text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
            Self::Utf8 => text.as_bytes().to_vec(),
        }
    }

    pub(crate) fn read(self, reader: &mut ByteReader<'_>) -> Result<String> {
        let length = reader.count(1, "bytes of text")?;
        Ok(self.decode(reader.take(length)?))
    }

    pub(crate) fn write(self, writer: &mut ByteWriter, text: &str) {
        let bytes = self.encode(text);
        writer.u32(bytes.len() as u32);
        writer.bytes(&bytes);
    }
}

fn decode_utf16(bytes: &[u8]) -> String {
    let units = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair));
    char::decode_utf16(units)
        .map(|decoded| decoded.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

pub(crate) fn decode_name(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0 || *byte == b'\n')
        .unwrap_or(bytes.len());
    let (text, _, _) = encoding_rs::SHIFT_JIS.decode(&bytes[..end]);
    text.into_owned()
}

pub(crate) fn encode_name(text: &str) -> Vec<u8> {
    let (bytes, _, _) = encoding_rs::SHIFT_JIS.encode(text);
    bytes.into_owned()
}
