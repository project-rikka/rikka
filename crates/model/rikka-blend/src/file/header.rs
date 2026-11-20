use rikka_model_core::{ByteReader, Endian, Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BHeadLayout {
    Narrow,
    Small,
    Large,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Header {
    pub(crate) pointer_size: usize,
    pub(crate) endian: Endian,
    pub(crate) version: u32,
    pub(crate) heads: BHeadLayout,
    pub(crate) length: usize,
}

pub(crate) const MAGIC: &[u8] = b"BLENDER";

const FORMAT: &str = ".blend";
const FILE_FORMAT: u32 = 1;
const LARGE_HEADER: u32 = 17;

pub(crate) fn decode(bytes: &[u8]) -> Result<Header> {
    let mut reader = ByteReader::new(bytes);
    if reader.take(MAGIC.len())? != MAGIC {
        return Err(Error::Damaged("missing .blend magic".to_owned()));
    }
    let marker = reader.u8()?;
    if let Some(pointer_size) = match marker {
        b'_' => Some(4),
        b'-' => Some(8),
        _ => None,
    } {
        let endian = match reader.u8()? {
            b'v' => Endian::Little,
            b'V' => Endian::Big,
            other => {
                return Err(Error::Damaged(format!(
                    "unknown .blend byte order mark {:?}",
                    other as char
                )));
            }
        };
        let version = digits(reader.take(3)?)?;
        return Ok(Header {
            pointer_size,
            endian,
            version,
            heads: match pointer_size {
                4 => BHeadLayout::Narrow,
                _ => BHeadLayout::Small,
            },
            length: reader.position(),
        });
    }
    let size = digits(&[marker, reader.u8()?])?;
    if size != LARGE_HEADER || reader.u8()? != b'-' {
        return Err(Error::Damaged("unknown .blend header layout".to_owned()));
    }
    let format = digits(reader.take(2)?)?;
    if format != FILE_FORMAT {
        return Err(Error::UnsupportedVersion {
            format: FORMAT,
            version: format!("file format {format}"),
        });
    }
    if reader.u8()? != b'v' {
        return Err(Error::Damaged("unknown .blend header layout".to_owned()));
    }
    Ok(Header {
        pointer_size: 8,
        endian: Endian::Little,
        version: digits(reader.take(4)?)?,
        heads: BHeadLayout::Large,
        length: reader.position(),
    })
}

fn digits(bytes: &[u8]) -> Result<u32> {
    bytes
        .iter()
        .try_fold(0, |value, &byte| match byte.is_ascii_digit() {
            true => Ok(value * 10 + u32::from(byte - b'0')),
            false => Err(Error::Damaged("unknown .blend version number".to_owned())),
        })
}
