use crate::bytes::Bytes;
use crate::error::{Error, Result};
use crate::fetch::Fetch;
use crate::format::Format;
use crate::layout::{Depth, Layout};
use crate::orientation::Orientation;
use crate::probe::Header;

const FORMAT: Format = Format::OpenExr;
const MAGIC: [u8; 4] = [0x76, 0x2F, 0x31, 0x01];
const FIRST_READ: u64 = 64 << 10;
const MOST_HEADER_BYTES: u64 = 16 << 20;
const DEEP: u32 = 0x800;
const MULTIPART: u32 = 0x1000;
const ONE_LEVEL: u8 = 0;
const OFFSET_BYTES: usize = 8;

type Window = (i32, i32, i32, i32);

#[derive(Default)]
struct Attributes {
    display: Option<Window>,
    data: Option<Window>,
    alpha: Option<bool>,
    compression: Option<u8>,
    tiles: Option<(u32, u32, u8)>,
    chunks: Option<u32>,
}

struct Headers {
    first: Attributes,
    multipart: bool,
    end: usize,
}

pub(crate) fn header(fetch: &mut Fetch<'_>) -> Result<Header> {
    let mut most = FIRST_READ;
    loop {
        let head = fetch.fetch_up_to(0, most)?;
        let whole = (head.len() as u64) < most;
        match headers(&head) {
            Ok(headers) => return finish(&headers.first),
            Err(Error::Truncated(_)) if !whole && most < MOST_HEADER_BYTES => most *= 4,
            Err(error) => return Err(error),
        }
    }
}

pub(crate) fn verify_chunks(data: &[u8]) -> Result<()> {
    let headers = headers(data)?;
    let Some(count) = chunk_count(&headers.first)? else {
        return Ok(());
    };
    let mut table = Bytes::new(FORMAT, data);
    table.seek(headers.end)?;
    let table_end = (count as usize)
        .checked_mul(OFFSET_BYTES)
        .and_then(|bytes| bytes.checked_add(headers.end))
        .filter(|&end| end <= data.len())
        .ok_or(Error::Truncated(FORMAT))?;
    let leading = match (headers.multipart, headers.first.tiles.is_some()) {
        (false, false) => 4,
        (false, true) => 16,
        (true, false) => 8,
        (true, true) => 20,
    };
    for _ in 0..count {
        let offset = usize::try_from(table.u64_le()?).map_err(|_| Error::Truncated(FORMAT))?;
        if offset < table_end {
            return Err(Error::malformed(FORMAT, "chunk offset table"));
        }
        let mut chunk = Bytes::new(FORMAT, data);
        chunk.seek(offset)?;
        chunk.skip(leading)?;
        let size = chunk.u32_le()? as usize;
        chunk.skip(size)?;
    }
    Ok(())
}

fn headers(head: &[u8]) -> Result<Headers> {
    let mut bytes = Bytes::new(FORMAT, head);
    if bytes.array::<4>()? != MAGIC {
        return Err(Error::malformed(FORMAT, "signature"));
    }
    let version = bytes.u32_le()?;
    if version & DEEP != 0 {
        return Err(Error::unsupported(FORMAT, "deep data"));
    }
    let multipart = version & MULTIPART != 0;
    let first = attributes(&mut bytes)?;
    if multipart {
        while bytes.rest().first() != Some(&0) {
            attributes(&mut bytes)?;
        }
        bytes.skip(1)?;
    }
    Ok(Headers {
        first,
        multipart,
        end: bytes.position(),
    })
}

fn attributes(bytes: &mut Bytes<'_>) -> Result<Attributes> {
    let mut attributes = Attributes::default();
    loop {
        let name = bytes.text()?;
        if name.is_empty() {
            return Ok(attributes);
        }
        let kind = bytes.text()?;
        let size = bytes.u32_le()? as usize;
        let mut value = Bytes::new(FORMAT, bytes.take(size)?);
        match (name, kind) {
            (b"displayWindow", b"box2i") => attributes.display = Some(window(&mut value)?),
            (b"dataWindow", b"box2i") => attributes.data = Some(window(&mut value)?),
            (b"channels", b"chlist") => attributes.alpha = Some(has_alpha(&mut value)?),
            (b"compression", b"compression") => attributes.compression = Some(value.u8()?),
            (b"tiles", b"tiledesc") => {
                attributes.tiles = Some((value.u32_le()?, value.u32_le()?, value.u8()? & 0xF));
            }
            (b"chunkCount", b"int") => attributes.chunks = Some(value.u32_le()?),
            _ => {}
        }
    }
}

fn window(value: &mut Bytes<'_>) -> Result<Window> {
    Ok((
        value.i32_le()?,
        value.i32_le()?,
        value.i32_le()?,
        value.i32_le()?,
    ))
}

fn has_alpha(list: &mut Bytes<'_>) -> Result<bool> {
    let mut alpha = false;
    loop {
        let name = list.text()?;
        if name.is_empty() {
            return Ok(alpha);
        }
        list.skip(16)?;
        alpha |= name == b"A";
    }
}

fn sides((left, top, right, bottom): Window) -> Option<(u32, u32)> {
    let side = |low: i32, high: i32| {
        u32::try_from(i64::from(high) - i64::from(low) + 1)
            .ok()
            .filter(|&side| side > 0)
    };
    Some((side(left, right)?, side(top, bottom)?))
}

fn chunk_count(attributes: &Attributes) -> Result<Option<u32>> {
    if let Some(chunks) = attributes.chunks {
        return Ok(Some(chunks));
    }
    let window = attributes
        .data
        .ok_or_else(|| Error::malformed(FORMAT, "data window"))?;
    let (width, height) = sides(window).ok_or_else(|| Error::malformed(FORMAT, "data window"))?;
    let count = match attributes.tiles {
        Some((tile_width, tile_height, ONE_LEVEL)) if tile_width > 0 && tile_height > 0 => {
            u64::from(width.div_ceil(tile_width)) * u64::from(height.div_ceil(tile_height))
        }
        Some(_) => return Ok(None),
        None => {
            let compression = attributes
                .compression
                .ok_or_else(|| Error::malformed(FORMAT, "compression"))?;
            let lines = match compression {
                0..=2 => 1,
                3 | 5 => 16,
                4 | 6..=8 => 32,
                9 => 256,
                _ => return Ok(None),
            };
            u64::from(height.div_ceil(lines))
        }
    };
    u32::try_from(count)
        .map(Some)
        .map_err(|_| Error::malformed(FORMAT, "chunk count"))
}

fn finish(attributes: &Attributes) -> Result<Header> {
    let window = attributes
        .display
        .ok_or_else(|| Error::malformed(FORMAT, "display window"))?;
    let alpha = attributes
        .alpha
        .ok_or_else(|| Error::malformed(FORMAT, "channel list"))?;
    let (width, height) =
        sides(window).ok_or_else(|| Error::malformed(FORMAT, "display window"))?;
    Ok(Header {
        width,
        height,
        layout: Layout::new(false, alpha, Depth::Float),
        orientation: Orientation::Upright,
        profile: None,
        premultiplied: alpha,
    })
}
