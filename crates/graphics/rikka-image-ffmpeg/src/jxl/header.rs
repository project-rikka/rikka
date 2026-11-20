use crate::error::{Error, Result};
use crate::format::Format;
use crate::jxl::bits::{Bits, Distribution};
use crate::layout::{Depth, Layout};
use crate::orientation::Orientation;

const FORMAT: Format = Format::JpegXl;
const SIGNATURE: u32 = 0x0AFF;
const MOST_EXTRA_CHANNELS: u32 = 256;
const MOST_DIMENSION_SHIFT: u32 = 3;
const DIMENSION: [Distribution; 4] = [
    Distribution::Bits(9, 1),
    Distribution::Bits(13, 1),
    Distribution::Bits(18, 1),
    Distribution::Bits(30, 1),
];
const PREVIEW_DIVIDED: [Distribution; 4] = [
    Distribution::Value(16),
    Distribution::Value(32),
    Distribution::Bits(5, 1),
    Distribution::Bits(9, 33),
];
const PREVIEW_DIMENSION: [Distribution; 4] = [
    Distribution::Bits(6, 1),
    Distribution::Bits(8, 65),
    Distribution::Bits(10, 321),
    Distribution::Bits(12, 1345),
];
const RATIOS: [(u64, u64); 7] = [(1, 1), (12, 10), (4, 3), (3, 2), (16, 9), (5, 4), (2, 1)];
const TICKS_NUMERATOR: [Distribution; 4] = [
    Distribution::Value(100),
    Distribution::Value(1000),
    Distribution::Bits(10, 1),
    Distribution::Bits(30, 1),
];
const TICKS_DENOMINATOR: [Distribution; 4] = [
    Distribution::Value(1),
    Distribution::Value(1001),
    Distribution::Bits(8, 1),
    Distribution::Bits(10, 1),
];
const LOOPS: [Distribution; 4] = [
    Distribution::Value(0),
    Distribution::Bits(3, 0),
    Distribution::Bits(16, 0),
    Distribution::Bits(32, 0),
];
const INTEGER_BITS: [Distribution; 4] = [
    Distribution::Value(8),
    Distribution::Value(10),
    Distribution::Value(12),
    Distribution::Bits(6, 1),
];
const FLOAT_BITS: [Distribution; 4] = [
    Distribution::Value(32),
    Distribution::Value(16),
    Distribution::Value(24),
    Distribution::Bits(6, 1),
];
const EXTRA_CHANNELS: [Distribution; 4] = [
    Distribution::Value(0),
    Distribution::Value(1),
    Distribution::Bits(4, 2),
    Distribution::Bits(12, 1),
];
const DIMENSION_SHIFT: [Distribution; 4] = [
    Distribution::Value(0),
    Distribution::Value(3),
    Distribution::Value(4),
    Distribution::Bits(3, 1),
];
const NAME_LENGTH: [Distribution; 4] = [
    Distribution::Value(0),
    Distribution::Bits(4, 0),
    Distribution::Bits(5, 16),
    Distribution::Bits(10, 48),
];
const FILTER_CHANNEL: [Distribution; 4] = [
    Distribution::Value(1),
    Distribution::Bits(2, 0),
    Distribution::Bits(4, 3),
    Distribution::Bits(8, 19),
];
const SPOT_COLOUR_BITS: u64 = 4 * 16;
const ALPHA_CHANNEL: u32 = 0;
const DEPTH_CHANNEL: u32 = 1;
const SPOT_COLOUR_CHANNEL: u32 = 2;
const SELECTION_MASK_CHANNEL: u32 = 3;
const BLACK_CHANNEL: u32 = 4;
const FILTER_ARRAY_CHANNEL: u32 = 5;
const THERMAL_CHANNEL: u32 = 6;
const OPTIONAL_CHANNEL: u32 = 16;
const GRAY_SPACE: u32 = 1;
const COLOUR_SPACES: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Alpha {
    Straight,
    Premultiplied,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Ticks {
    pub(crate) numerator: u32,
    pub(crate) denominator: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Timing {
    pub(crate) ticks: Ticks,
    pub(crate) plays: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ImageHeader {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) orientation: Orientation,
    pub(crate) depth: Depth,
    pub(crate) gray: bool,
    pub(crate) alpha: Option<Alpha>,
    pub(crate) timing: Option<Timing>,
}

impl ImageHeader {
    pub(crate) fn read(bytes: &[u8]) -> Result<ImageHeader> {
        let mut bits = Bits::new(bytes);
        if bits.read(16)? != SIGNATURE {
            return Err(Error::malformed(FORMAT, "codestream signature"));
        }
        let (width, height) = size(&mut bits)?;
        let mut header = ImageHeader {
            width,
            height,
            orientation: Orientation::Upright,
            depth: Depth::Eight,
            gray: false,
            alpha: None,
            timing: None,
        };
        if bits.flag()? {
            return Ok(header);
        }
        if bits.flag()? {
            let orientation = bits.read(3)? + 1;
            header.orientation = Orientation::from_exif_value(orientation as u16)
                .unwrap_or_else(|| unreachable!("three bits name one of eight orientations"));
            if bits.flag()? {
                size(&mut bits)?;
            }
            if bits.flag()? {
                preview(&mut bits)?;
            }
            if bits.flag()? {
                header.timing = Some(timing(&mut bits)?);
            }
        }
        header.depth = bit_depth(&mut bits)?;
        bits.flag()?;
        let extra = bits.choose(EXTRA_CHANNELS)?;
        if extra > MOST_EXTRA_CHANNELS {
            return Err(Error::unsupported(FORMAT, "more than 256 extra channels"));
        }
        for _ in 0..extra {
            match extra_channel(&mut bits)? {
                ExtraChannel::Alpha(alpha) => {
                    header.alpha.get_or_insert(alpha);
                }
                ExtraChannel::Black => return Err(Error::unsupported(FORMAT, "CMYK pictures")),
                ExtraChannel::Other => {}
            }
        }
        bits.flag()?;
        header.gray = is_gray(&mut bits)?;
        Ok(header)
    }

    pub(crate) fn layout(&self) -> Layout {
        Layout::new(self.gray, self.alpha.is_some(), self.depth)
    }

    pub(crate) fn displayed(&self) -> (u32, u32) {
        match self.orientation {
            Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipHorizontal
            | Orientation::Rotate270FlipHorizontal => (self.height, self.width),
            Orientation::Upright
            | Orientation::Rotate180
            | Orientation::FlipHorizontal
            | Orientation::FlipVertical => (self.width, self.height),
        }
    }
}

fn size(bits: &mut Bits<'_>) -> Result<(u32, u32)> {
    let small = bits.flag()?;
    let side = |bits: &mut Bits<'_>| match small {
        true => Ok((bits.read(5)? + 1) * 8),
        false => bits.choose(DIMENSION),
    };
    let height = side(bits)?;
    let ratio = bits.read(3)?;
    let width = match ratio {
        0 => side(bits)?,
        ratio => scaled(height, ratio)?,
    };
    Ok((width, height))
}

fn scaled(height: u32, ratio: u32) -> Result<u32> {
    let (numerator, denominator) = RATIOS[ratio as usize - 1];
    u32::try_from(u64::from(height) * numerator / denominator)
        .map_err(|_| Error::malformed(FORMAT, "image size"))
}

fn preview(bits: &mut Bits<'_>) -> Result<()> {
    let divided = bits.flag()?;
    let side = |bits: &mut Bits<'_>| match divided {
        true => bits.choose(PREVIEW_DIVIDED),
        false => bits.choose(PREVIEW_DIMENSION),
    };
    side(bits)?;
    if bits.read(3)? == 0 {
        side(bits)?;
    }
    Ok(())
}

fn timing(bits: &mut Bits<'_>) -> Result<Timing> {
    let ticks = Ticks {
        numerator: bits.choose(TICKS_NUMERATOR)?,
        denominator: bits.choose(TICKS_DENOMINATOR)?,
    };
    let plays = match bits.choose(LOOPS)? {
        0 => None,
        plays => Some(plays),
    };
    bits.flag()?;
    Ok(Timing { ticks, plays })
}

fn bit_depth(bits: &mut Bits<'_>) -> Result<Depth> {
    if !bits.flag()? {
        return match bits.choose(INTEGER_BITS)? {
            1..=8 => Ok(Depth::Eight),
            9..=16 => Ok(Depth::Sixteen),
            17..=31 => Ok(Depth::Float),
            _ => Err(Error::malformed(FORMAT, "bit depth")),
        };
    }
    let sample = bits.choose(FLOAT_BITS)?;
    let exponent = bits.read(4)? + 1;
    let mantissa = sample.checked_sub(exponent + 1);
    match (exponent, mantissa) {
        (2..=8, Some(2..=23)) => Ok(Depth::Float),
        _ => Err(Error::malformed(FORMAT, "bit depth")),
    }
}

enum ExtraChannel {
    Alpha(Alpha),
    Black,
    Other,
}

fn extra_channel(bits: &mut Bits<'_>) -> Result<ExtraChannel> {
    if bits.flag()? {
        return Ok(ExtraChannel::Alpha(Alpha::Straight));
    }
    let kind = bits.enumerated()?;
    bit_depth(bits)?;
    if bits.choose(DIMENSION_SHIFT)? > MOST_DIMENSION_SHIFT {
        return Err(Error::malformed(FORMAT, "extra channel"));
    }
    let name = bits.choose(NAME_LENGTH)?;
    bits.skip(u64::from(name) * 8)?;
    match kind {
        ALPHA_CHANNEL => Ok(ExtraChannel::Alpha(match bits.flag()? {
            true => Alpha::Premultiplied,
            false => Alpha::Straight,
        })),
        SPOT_COLOUR_CHANNEL => bits.skip(SPOT_COLOUR_BITS).map(|()| ExtraChannel::Other),
        BLACK_CHANNEL => Ok(ExtraChannel::Black),
        FILTER_ARRAY_CHANNEL => bits.choose(FILTER_CHANNEL).map(|_| ExtraChannel::Other),
        DEPTH_CHANNEL | SELECTION_MASK_CHANNEL | THERMAL_CHANNEL | OPTIONAL_CHANNEL => {
            Ok(ExtraChannel::Other)
        }
        _ => Err(Error::malformed(FORMAT, "extra channel")),
    }
}

fn is_gray(bits: &mut Bits<'_>) -> Result<bool> {
    if bits.flag()? {
        return Ok(false);
    }
    bits.flag()?;
    match bits.enumerated()? {
        GRAY_SPACE => Ok(true),
        space if space < COLOUR_SPACES => Ok(false),
        _ => Err(Error::malformed(FORMAT, "colour encoding")),
    }
}
