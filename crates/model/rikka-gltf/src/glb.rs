use rikka_model_core::{ByteReader, ByteWriter, Error, Result};

pub(crate) const MAGIC: [u8; 4] = *b"glTF";

const JSON_CHUNK: u32 = 0x4E4F_534A;

const BINARY_CHUNK: u32 = 0x004E_4942;

const HEADER: usize = 12;

const CHUNK_HEADER: usize = 8;

const ALIGNMENT: usize = 4;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Chunks<'a> {
    pub json: &'a [u8],
    pub binary: Option<&'a [u8]>,
}

impl<'a> Chunks<'a> {
    pub(crate) fn split(bytes: &'a [u8]) -> Result<Self> {
        match bytes.starts_with(&MAGIC) {
            true => Self::binary(bytes),
            false => Ok(Self {
                json: bytes,
                binary: None,
            }),
        }
    }

    fn binary(bytes: &'a [u8]) -> Result<Self> {
        let mut reader = ByteReader::new(bytes);
        let truncated = |_| damaged("truncated header");
        reader.take(MAGIC.len()).map_err(truncated)?;
        let version = reader.u32().map_err(truncated)?;
        if version != 2 {
            return Err(Error::UnsupportedVersion {
                format: "GLB",
                version: version.to_string(),
            });
        }
        let length = reader.u32().map_err(truncated)? as usize;
        if length > bytes.len() {
            return Err(damaged(&format!(
                "GLB header declares {length} bytes, file has {}",
                bytes.len()
            )));
        }

        let mut json = None;
        let mut binary = None;
        while reader.position() + CHUNK_HEADER <= length {
            let size = reader.u32()? as usize;
            let kind = reader.u32()?;
            let end = reader
                .position()
                .checked_add(size)
                .filter(|end| *end <= length)
                .ok_or_else(|| damaged("chunk extends past end of file"))?;
            let chunk = reader.take(size)?;
            match kind {
                JSON_CHUNK if json.is_none() => json = Some(chunk),
                BINARY_CHUNK if binary.is_none() => binary = Some(chunk),
                _ => {}
            }
            reader.seek(end.next_multiple_of(ALIGNMENT).min(length))?;
        }

        Ok(Self {
            json: json.ok_or_else(|| damaged("missing JSON chunk"))?,
            binary,
        })
    }
}

#[must_use]
pub(crate) fn wrap(json: &[u8], binary: &[u8]) -> Vec<u8> {
    let json_length = json.len().next_multiple_of(ALIGNMENT);
    let binary_length = binary.len().next_multiple_of(ALIGNMENT);
    let mut chunks = HEADER + CHUNK_HEADER + json_length;
    if !binary.is_empty() {
        chunks += CHUNK_HEADER + binary_length;
    }

    let mut writer = ByteWriter::with_capacity(chunks);
    writer.bytes(&MAGIC);
    writer.u32(2);
    writer.u32(chunks as u32);

    writer.u32(json_length as u32);
    writer.u32(JSON_CHUNK);
    writer.bytes(json);
    writer.align(ALIGNMENT, b' ');

    if !binary.is_empty() {
        writer.u32(binary_length as u32);
        writer.u32(BINARY_CHUNK);
        writer.bytes(binary);
        writer.align(ALIGNMENT, 0);
    }
    writer.finish()
}

fn damaged(reason: &str) -> Error {
    Error::Damaged(format!("invalid GLB: {reason}"))
}
