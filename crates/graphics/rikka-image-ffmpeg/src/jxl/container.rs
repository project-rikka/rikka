use crate::atoms::top_level;
use crate::error::{Error, Result};
use crate::fetch::Fetch;
use crate::format::Format;
use crate::jxl::header::ImageHeader;

const FORMAT: Format = Format::JpegXl;
const CODESTREAM: [u8; 2] = [0xFF, 0x0A];
const SIGNATURE_BOX: [u8; 12] = *b"\0\0\0\x0CJXL \r\n\x87\n";
const BRAND: [u8; 4] = *b"jxl ";
const MOST_HEADER_BYTES: u64 = 1 << 20;
const PART_INDEX_BYTES: u64 = 4;
const KIND_BYTES: u64 = 4;
const LAST_PART: u32 = 1 << 31;
const UNCOMPRESSIBLE: [[u8; 4]; 8] = [
    *b"JXL ", *b"ftyp", *b"jxlc", *b"jxlp", *b"jxll", *b"jxli", *b"jbrd", *b"brob",
];

pub(crate) fn image_header(fetch: &mut Fetch<'_>) -> Result<ImageHeader> {
    let head = codestream_head(fetch)?;
    ImageHeader::read(&head).map_err(|error| match error {
        Error::Truncated(_) if head.len() as u64 >= MOST_HEADER_BYTES => {
            Error::unsupported(FORMAT, "image headers above 1 MiB")
        }
        other => other,
    })
}

fn codestream_head(fetch: &mut Fetch<'_>) -> Result<Vec<u8>> {
    let start = fetch
        .fetch_up_to(0, SIGNATURE_BOX.len() as u64)?
        .into_owned();
    if start.starts_with(&CODESTREAM) {
        return Ok(fetch.fetch_up_to(0, MOST_HEADER_BYTES)?.into_owned());
    }
    if start != SIGNATURE_BOX {
        return Err(match SIGNATURE_BOX.starts_with(&start) {
            true => Error::Truncated(FORMAT),
            false => Error::malformed(FORMAT, "signature"),
        });
    }
    let spans = top_level(fetch)?;
    let brand = spans
        .get(1)
        .filter(|span| span.kind == *b"ftyp" && span.len >= BRAND.len() as u64)
        .ok_or_else(|| Error::malformed(FORMAT, "file type box"))?;
    if fetch.fetch(brand.at, BRAND.len() as u64)?.as_ref() != BRAND {
        return Err(Error::malformed(FORMAT, "file type brand"));
    }
    let mut head = Vec::new();
    let mut whole = false;
    let mut parts = 0u32;
    for span in &spans[2..] {
        let room = MOST_HEADER_BYTES - head.len() as u64;
        match &span.kind {
            b"jxlc" if parts == 0 && !whole => {
                head.extend_from_slice(&fetch.fetch(span.at, span.len.min(room))?);
                whole = true;
            }
            b"jxlp" if !whole => {
                let body = span
                    .len
                    .checked_sub(PART_INDEX_BYTES)
                    .ok_or_else(|| Error::malformed(FORMAT, "partial codestream box"))?;
                let index = fetch.fetch(span.at, PART_INDEX_BYTES)?;
                let index = u32::from_be_bytes([index[0], index[1], index[2], index[3]]);
                if index & !LAST_PART != parts {
                    return Err(Error::malformed(FORMAT, "partial codestream order"));
                }
                parts += 1;
                whole = index & LAST_PART != 0;
                head.extend_from_slice(&fetch.fetch(span.at + PART_INDEX_BYTES, body.min(room))?);
            }
            b"jxlc" | b"jxlp" => return Err(Error::malformed(FORMAT, "codestream boxes")),
            b"brob" => {
                let inner = match span.len >= KIND_BYTES {
                    true => fetch.fetch(span.at, KIND_BYTES)?,
                    false => return Err(Error::malformed(FORMAT, "compressed box")),
                };
                if UNCOMPRESSIBLE.iter().any(|kind| kind == inner.as_ref()) {
                    return Err(Error::malformed(FORMAT, "compressed box"));
                }
            }
            b"JXL " | b"ftyp" => return Err(Error::malformed(FORMAT, "repeated header box")),
            _ => {}
        }
        if head.len() as u64 >= MOST_HEADER_BYTES {
            break;
        }
    }
    match head.is_empty() {
        true => Err(Error::malformed(FORMAT, "file without a codestream")),
        false => Ok(head),
    }
}
