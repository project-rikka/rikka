use crate::codec::{Codec, Reader, Writer};
use crate::error::DecodeError;
use crate::version::VERSION;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BinaryFormat {
    name: &'static str,
    magic: [u8; 8],
}

impl BinaryFormat {
    pub const HEADER_LEN: usize = 12;

    #[must_use]
    pub const fn new(name: &'static str, magic: [u8; 8]) -> BinaryFormat {
        BinaryFormat { name, magic }
    }

    #[must_use]
    pub fn header(&self) -> [u8; BinaryFormat::HEADER_LEN] {
        let mut header = [0; BinaryFormat::HEADER_LEN];
        header[..8].copy_from_slice(&self.magic);
        header[8..].copy_from_slice(&VERSION.to_le_bytes());
        header
    }

    pub fn write_header(&self, out: &mut Writer<'_>) {
        out.raw(&self.header());
    }

    pub fn read_header(&self, input: &mut Reader<'_>) -> Result<(), DecodeError> {
        let foreign = DecodeError::Foreign {
            expected: self.name,
        };
        let header: [u8; BinaryFormat::HEADER_LEN] = input.array().map_err(|_| foreign)?;
        if header[..8] != self.magic {
            return Err(foreign);
        }
        let found = u32::from_le_bytes(header[8..].try_into().expect("four version bytes"));
        match found == VERSION {
            true => Ok(()),
            false => Err(DecodeError::Version {
                format: self.name,
                found,
            }),
        }
    }

    #[must_use]
    pub fn encode<T: Codec>(&self, value: &T) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut out = Writer::new(&mut bytes);
        self.write_header(&mut out);
        out.put(value);
        bytes
    }

    pub fn decode<T: Codec>(&self, bytes: &[u8]) -> Result<T, T::Error> {
        let mut input = Reader::new(bytes);
        self.read_header(&mut input)?;
        let value = input.get::<T>()?;
        input.finish()?;
        Ok(value)
    }
}
