use crate::parse::array::{Array, Scalar};
use crate::parse::bytes::Bytes;
use crate::sfnt::glyph_id::GlyphId;

const SMALL_METRICS_SIZE: usize = 5;
const BIG_METRICS_SIZE: usize = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BitmapMetrics {
    pub width: u8,
    pub height: u8,
    pub bearing_x: i8,
    pub bearing_y: i8,
    pub advance: u8,
}

impl BitmapMetrics {
    pub(crate) fn small(data: &[u8]) -> Option<Self> {
        let [height, width, bearing_x, bearing_y, advance] = data.chunk::<SMALL_METRICS_SIZE>(0)?;
        Some(Self {
            width,
            height,
            bearing_x: bearing_x as i8,
            bearing_y: bearing_y as i8,
            advance,
        })
    }

    pub(crate) fn big(data: &[u8]) -> Option<Self> {
        Self::small(data.range(0, BIG_METRICS_SIZE)?)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BitmapComponent {
    pub glyph: GlyphId,
    pub x_offset: i8,
    pub y_offset: i8,
}

impl Scalar for BitmapComponent {
    const SIZE: usize = 4;

    #[inline]
    fn parse(bytes: &[u8]) -> Option<Self> {
        let [high, low, x_offset, y_offset] = bytes.chunk::<4>(0)?;
        Some(Self {
            glyph: GlyphId(u16::from_be_bytes([high, low])),
            x_offset: x_offset as i8,
            y_offset: y_offset as i8,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BitmapData<'a> {
    Png(&'a [u8]),
    Mask { data: &'a [u8], bit_aligned: bool },
    Composite(Array<'a, BitmapComponent>),
}

#[derive(Clone, Copy, Debug)]
pub struct BitmapGlyph<'a> {
    pub ppem_x: u8,
    pub ppem_y: u8,
    pub bit_depth: u8,
    pub metrics: BitmapMetrics,
    pub data: BitmapData<'a>,
}

pub(crate) struct ImageLocation<'a> {
    pub(crate) format: u16,
    pub(crate) data: &'a [u8],
    pub(crate) metrics: Option<BitmapMetrics>,
}

pub(crate) fn decode_image(
    image: ImageLocation<'_>,
    bit_depth: u8,
) -> Option<(BitmapMetrics, BitmapData<'_>)> {
    let data = image.data;
    match image.format {
        1 | 2 => {
            let metrics = BitmapMetrics::small(data)?;
            let mask = mask(
                data.tail(SMALL_METRICS_SIZE)?,
                metrics,
                bit_depth,
                image.format == 2,
            )?;
            Some((metrics, mask))
        }
        5 => {
            let metrics = image.metrics?;
            Some((metrics, mask(data, metrics, bit_depth, true)?))
        }
        6 | 7 => {
            let metrics = BitmapMetrics::big(data)?;
            let mask = mask(
                data.tail(BIG_METRICS_SIZE)?,
                metrics,
                bit_depth,
                image.format == 7,
            )?;
            Some((metrics, mask))
        }
        8 => {
            let metrics = BitmapMetrics::small(data)?;
            Some((metrics, components(data, SMALL_METRICS_SIZE + 1)?))
        }
        9 => {
            let metrics = BitmapMetrics::big(data)?;
            Some((metrics, components(data, BIG_METRICS_SIZE)?))
        }
        17 => Some((BitmapMetrics::small(data)?, png(data, SMALL_METRICS_SIZE)?)),
        18 => Some((BitmapMetrics::big(data)?, png(data, BIG_METRICS_SIZE)?)),
        19 => Some((image.metrics?, png(data, 0)?)),
        _ => None,
    }
}

fn mask(
    data: &[u8],
    metrics: BitmapMetrics,
    bit_depth: u8,
    bit_aligned: bool,
) -> Option<BitmapData<'_>> {
    if !matches!(bit_depth, 1 | 2 | 4 | 8) {
        return None;
    }
    let (width, height, depth) = (
        usize::from(metrics.width),
        usize::from(metrics.height),
        usize::from(bit_depth),
    );
    let len = if bit_aligned {
        (width * height * depth).div_ceil(8)
    } else {
        height * (width * depth).div_ceil(8)
    };
    Some(BitmapData::Mask {
        data: data.range(0, len)?,
        bit_aligned,
    })
}

fn components(data: &[u8], at: usize) -> Option<BitmapData<'_>> {
    let count = usize::from(data.u16_at(at)?);
    Some(BitmapData::Composite(data.array(at + 2, count)?))
}

fn png(data: &[u8], at: usize) -> Option<BitmapData<'_>> {
    let len = data.u32_at(at)? as usize;
    Some(BitmapData::Png(data.range(at + 4, len)?))
}
