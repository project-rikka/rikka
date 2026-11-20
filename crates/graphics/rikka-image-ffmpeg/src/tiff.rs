use crate::error::{Error, Result};
use crate::fetch::Fetch;
use crate::format::Format;
use crate::layout::{Depth, Layout};
use crate::orientation::Orientation;
use crate::probe::Header;

const FORMAT: Format = Format::Tiff;
const ENTRY_BYTES: u64 = 12;
const MOST_ENTRIES: u64 = 4096;
const MOST_PROFILE_BYTES: u64 = 16 << 20;
const BIG_TIFF: u16 = 43;
const CLASSIC: u16 = 42;

const WIDTH: u16 = 256;
const HEIGHT: u16 = 257;
const BITS_PER_SAMPLE: u16 = 258;
const PHOTOMETRIC: u16 = 262;
const ORIENTATION: u16 = 274;
const SAMPLES_PER_PIXEL: u16 = 277;
const EXTRA_SAMPLES: u16 = 338;
const SAMPLE_FORMAT: u16 = 339;
const ICC_PROFILE: u16 = 34675;

const SHORT: u16 = 3;
const LONG: u16 = 4;
const UNDEFINED: u16 = 7;

const PALETTE: u32 = 3;
const SEPARATED: u32 = 5;
const WHITE_IS_ZERO: u32 = 0;
const BLACK_IS_ZERO: u32 = 1;
const FLOATING_POINT: u32 = 3;
const ASSOCIATED_ALPHA: u32 = 1;

#[derive(Clone, Copy, Debug)]
struct Order {
    big_endian: bool,
}

impl Order {
    fn u16(self, bytes: &[u8]) -> u16 {
        let pair = [bytes[0], bytes[1]];
        match self.big_endian {
            true => u16::from_be_bytes(pair),
            false => u16::from_le_bytes(pair),
        }
    }

    fn u32(self, bytes: &[u8]) -> u32 {
        let quad = [bytes[0], bytes[1], bytes[2], bytes[3]];
        match self.big_endian {
            true => u32::from_be_bytes(quad),
            false => u32::from_le_bytes(quad),
        }
    }

    fn put_u16(self, value: u16) -> [u8; 2] {
        match self.big_endian {
            true => value.to_be_bytes(),
            false => value.to_le_bytes(),
        }
    }

    fn put_u32(self, value: u32) -> [u8; 4] {
        match self.big_endian {
            true => value.to_be_bytes(),
            false => value.to_le_bytes(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    tag: u16,
    kind: u16,
    count: u32,
    value: [u8; 4],
}

impl Entry {
    fn scalar(self, order: Order) -> Option<u32> {
        match self.kind {
            SHORT => Some(u32::from(order.u16(&self.value))),
            LONG => Some(order.u32(&self.value)),
            _ => None,
        }
    }
}

pub(crate) fn header(fetch: &mut Fetch<'_>) -> Result<Header> {
    let head = fetch.fetch(0, 8)?;
    let order = match &head[..2] {
        b"II" => Order { big_endian: false },
        b"MM" => Order { big_endian: true },
        _ => return Err(Error::malformed(FORMAT, "byte order")),
    };
    match order.u16(&head[2..4]) {
        CLASSIC => {}
        BIG_TIFF => return Err(Error::unsupported(FORMAT, "BigTIFF")),
        _ => return Err(Error::malformed(FORMAT, "signature")),
    }
    let directory = u64::from(order.u32(&head[4..8]));
    let count = u64::from(order.u16(&fetch.fetch(directory, 2)?));
    if count == 0 || count > MOST_ENTRIES {
        return Err(Error::malformed(FORMAT, "image directory"));
    }
    let table = fetch
        .fetch(directory + 2, count * ENTRY_BYTES)?
        .into_owned();
    let entries: Vec<Entry> = table
        .as_chunks::<{ ENTRY_BYTES as usize }>()
        .0
        .iter()
        .map(|raw| Entry {
            tag: order.u16(&raw[0..2]),
            kind: order.u16(&raw[2..4]),
            count: order.u32(&raw[4..8]),
            value: [raw[8], raw[9], raw[10], raw[11]],
        })
        .collect();
    let find = |tag: u16| entries.iter().find(|entry| entry.tag == tag).copied();
    let scalar = |tag: u16| find(tag).and_then(|entry| entry.scalar(order));
    let width = scalar(WIDTH).ok_or_else(|| Error::malformed(FORMAT, "image width"))?;
    let height = scalar(HEIGHT).ok_or_else(|| Error::malformed(FORMAT, "image height"))?;
    let samples = scalar(SAMPLES_PER_PIXEL).unwrap_or(1);
    let photometric = scalar(PHOTOMETRIC).unwrap_or(BLACK_IS_ZERO);
    let bits = match find(BITS_PER_SAMPLE) {
        Some(entry) if entry.count <= 2 => entry.scalar(order).unwrap_or(1),
        Some(entry) if entry.kind == SHORT => {
            let at = u64::from(order.u32(&entry.value));
            order.u16(&fetch.fetch(at, 2)?).into()
        }
        _ => 1,
    };
    let floating = scalar(SAMPLE_FORMAT) == Some(FLOATING_POINT);
    let gray = matches!(photometric, WHITE_IS_ZERO | BLACK_IS_ZERO) || samples == 2;
    let alpha = match photometric {
        PALETTE => false,
        SEPARATED => samples > 4,
        _ if gray => samples > 1,
        _ => samples > 3,
    };
    let depth = match (floating, bits) {
        (true, _) => Depth::Float,
        (false, 0..=8) => Depth::Eight,
        (false, _) => Depth::Sixteen,
    };
    let gray = gray && photometric != PALETTE;
    let orientation = scalar(ORIENTATION)
        .and_then(|value| u16::try_from(value).ok())
        .and_then(Orientation::from_exif_value)
        .unwrap_or_default();
    let profile = match find(ICC_PROFILE) {
        Some(entry) if u64::from(entry.count) <= MOST_PROFILE_BYTES && entry.count > 4 => {
            let at = u64::from(order.u32(&entry.value));
            Some(fetch.fetch(at, u64::from(entry.count))?.into_owned())
        }
        _ => None,
    };
    Ok(Header {
        width,
        height,
        layout: Layout::new(gray, alpha, depth),
        orientation,
        profile,
        premultiplied: alpha && scalar(EXTRA_SAMPLES) == Some(ASSOCIATED_ALPHA),
    })
}

pub(crate) fn with_profile(file: &[u8], profile: &[u8]) -> Result<Vec<u8>> {
    let malformed = || Error::malformed(FORMAT, "encoded image directory");
    let order = match file.get(..2) {
        Some(b"II") => Order { big_endian: false },
        Some(b"MM") => Order { big_endian: true },
        _ => return Err(malformed()),
    };
    let directory = order.u32(file.get(4..8).ok_or_else(malformed)?) as usize;
    let count = usize::from(order.u16(file.get(directory..directory + 2).ok_or_else(malformed)?));
    let table = directory + 2;
    let table_end = table + count * ENTRY_BYTES as usize;
    let entries = file.get(table..table_end).ok_or_else(malformed)?;
    let next = file.get(table_end..table_end + 4).ok_or_else(malformed)?;
    let wide = |value: usize| {
        u32::try_from(value).map_err(|_| Error::unsupported(FORMAT, "files over 4 GiB"))
    };
    let mut out = file.to_vec();
    out.resize(out.len().next_multiple_of(2), 0);
    let profile_at = wide(out.len())?;
    out.extend_from_slice(profile);
    out.resize(out.len().next_multiple_of(2), 0);
    let mut kept: Vec<[u8; ENTRY_BYTES as usize]> = entries
        .as_chunks::<{ ENTRY_BYTES as usize }>()
        .0
        .iter()
        .filter(|raw| order.u16(&raw[0..2]) != ICC_PROFILE)
        .copied()
        .collect();
    let mut icc = [0; ENTRY_BYTES as usize];
    icc[0..2].copy_from_slice(&order.put_u16(ICC_PROFILE));
    icc[2..4].copy_from_slice(&order.put_u16(UNDEFINED));
    icc[4..8].copy_from_slice(&order.put_u32(wide(profile.len())?));
    icc[8..12].copy_from_slice(&order.put_u32(profile_at));
    let place = kept
        .iter()
        .position(|raw| order.u16(&raw[0..2]) > ICC_PROFILE)
        .unwrap_or(kept.len());
    kept.insert(place, icc);
    let directory_at = wide(out.len())?;
    let entry_count = u16::try_from(kept.len()).map_err(|_| malformed())?;
    out.extend_from_slice(&order.put_u16(entry_count));
    for entry in &kept {
        out.extend_from_slice(entry);
    }
    out.extend_from_slice(next);
    out[4..8].copy_from_slice(&order.put_u32(directory_at));
    Ok(out)
}
