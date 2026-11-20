use crate::atoms::{Atoms, top_level};
use crate::bytes::Bytes;
use crate::error::{Error, Result};
use crate::fetch::Fetch;
use crate::format::Format;
use crate::layout::{Depth, Layout};
use crate::orientation::Orientation;
use crate::probe::Header;

const FORMAT: Format = Format::Jpeg2000;
const CODESTREAM: [u8; 4] = [0xFF, 0x4F, 0xFF, 0x51];
const SIZE_SEGMENT_BYTES: u64 = 4 + 2 + 36;
const COMPONENT_BYTES: u64 = 3;
const MOST_HEADER_BYTES: u64 = 16 << 20;
const RESTRICTED_PROFILE: u8 = 2;
const ANY_PROFILE: u8 = 3;

pub(crate) fn header(fetch: &mut Fetch<'_>) -> Result<Header> {
    let signature = fetch.fetch_up_to(0, 4)?;
    let (start, profile) = match signature.as_ref() == CODESTREAM {
        true => (0, None),
        false => boxed(fetch)?,
    };
    let (width, height, layout) = size(fetch, start)?;
    Ok(Header {
        width,
        height,
        layout,
        orientation: Orientation::Upright,
        profile,
        premultiplied: false,
    })
}

fn boxed(fetch: &mut Fetch<'_>) -> Result<(u64, Option<Vec<u8>>)> {
    let mut profile = None;
    for span in top_level(fetch)? {
        match &span.kind {
            b"jp2h" if span.len <= MOST_HEADER_BYTES => {
                let body = fetch.fetch(span.at, span.len)?;
                for atom in Atoms::new(FORMAT, &body) {
                    let atom = atom?;
                    if atom.kind != *b"colr" {
                        continue;
                    }
                    let mut colour = Bytes::new(FORMAT, atom.body);
                    let method = colour.u8()?;
                    colour.skip(2)?;
                    if matches!(method, RESTRICTED_PROFILE | ANY_PROFILE) {
                        profile = Some(colour.rest().to_vec());
                    }
                }
            }
            b"jp2c" => return Ok((span.at, profile)),
            _ => {}
        }
    }
    Err(Error::malformed(FORMAT, "file without a codestream"))
}

fn size(fetch: &mut Fetch<'_>, start: u64) -> Result<(u32, u32, Layout)> {
    let fixed = fetch.fetch(start, SIZE_SEGMENT_BYTES)?.into_owned();
    let mut bytes = Bytes::new(FORMAT, &fixed);
    if bytes.array::<4>()? != CODESTREAM {
        return Err(Error::malformed(FORMAT, "codestream signature"));
    }
    bytes.skip(4)?;
    let (right, bottom) = (bytes.u32_be()?, bytes.u32_be()?);
    let (left, top) = (bytes.u32_be()?, bytes.u32_be()?);
    bytes.skip(16)?;
    let components = bytes.u16_be()?;
    if components == 0 {
        return Err(Error::malformed(FORMAT, "image size segment"));
    }
    let listed = fetch.fetch(
        start + SIZE_SEGMENT_BYTES,
        u64::from(components) * COMPONENT_BYTES,
    )?;
    let deepest = listed
        .as_chunks::<{ COMPONENT_BYTES as usize }>()
        .0
        .iter()
        .map(|component| (component[0] & 0x7F) + 1)
        .max()
        .unwrap_or_else(|| unreachable!("the size segment lists at least one component"));
    let (Some(width), Some(height)) = (right.checked_sub(left), bottom.checked_sub(top)) else {
        return Err(Error::malformed(FORMAT, "image size segment"));
    };
    let depth = match deepest {
        0..=8 => Depth::Eight,
        _ => Depth::Sixteen,
    };
    let layout = Layout::new(components < 3, components == 2 || components > 3, depth);
    Ok((width, height, layout))
}
