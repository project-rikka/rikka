use crate::bytes::Bytes;
use crate::error::{Error, Result};
use crate::fetch::Fetch;
use crate::format::Format;
use crate::heif::meta::{Aperture, Meta, Nclx, Property};
use crate::layout::{Depth, Layout};
use crate::orientation::Orientation;

const GRID_DESCRIPTOR_BYTES: u64 = 12;
const ALPHA_TYPES: [&[u8]; 2] = [
    b"urn:mpeg:mpegB:cicp:systems:auxiliary:alpha",
    b"urn:mpeg:hevc:2015:auxid:1",
];
const AV1_HIGH_BITDEPTH: u8 = 0x40;
const AV1_TWELVE_BIT: u8 = 0x20;
const AV1_MONOCHROME: u8 = 0x10;
const HEVC_CHROMA_FORMAT: usize = 16;
const HEVC_LUMA_DEPTH: usize = 17;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Codec {
    Av1,
    Hevc,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Coding {
    pub(crate) codec: Codec,
    pub(crate) config: Vec<u8>,
    pub(crate) depth: u8,
    pub(crate) monochrome: bool,
}

impl Coding {
    pub(crate) fn new(format: Format, kind: [u8; 4], config: &[u8]) -> Result<Coding> {
        let codec = match &kind {
            b"av01" => Codec::Av1,
            b"hvc1" | b"hev1" => Codec::Hevc,
            _ => return Err(Error::unsupported(format, "image coding")),
        };
        let malformed = || Error::malformed(format, "codec configuration");
        let (depth, monochrome) = match codec {
            Codec::Av1 => {
                let flags = *config.get(2).ok_or_else(malformed)?;
                let depth = match (flags & AV1_HIGH_BITDEPTH != 0, flags & AV1_TWELVE_BIT != 0) {
                    (false, _) => 8,
                    (true, false) => 10,
                    (true, true) => 12,
                };
                (depth, flags & AV1_MONOCHROME != 0)
            }
            Codec::Hevc => {
                let chroma = *config.get(HEVC_CHROMA_FORMAT).ok_or_else(malformed)? & 0x3;
                let depth = (*config.get(HEVC_LUMA_DEPTH).ok_or_else(malformed)? & 0x7) + 8;
                (depth, chroma == 0)
            }
        };
        Ok(Coding {
            codec,
            config: config.to_vec(),
            depth,
            monochrome,
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Tiling {
    Single(u32),
    Grid {
        columns: u32,
        tile_width: u32,
        tile_height: u32,
        tiles: Vec<u32>,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct Plane {
    pub(crate) coding: Coding,
    pub(crate) nclx: Option<Nclx>,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) tiling: Tiling,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Crop {
    pub(crate) left: u32,
    pub(crate) top: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct Still {
    pub(crate) color: Plane,
    pub(crate) alpha: Option<Plane>,
    pub(crate) premultiplied: bool,
    pub(crate) profile: Option<Vec<u8>>,
    pub(crate) orientation: Orientation,
    pub(crate) crop: Option<Crop>,
}

impl Still {
    pub(crate) fn read(meta: &Meta, fetch: &mut Fetch<'_>) -> Result<Still> {
        let format = meta.format;
        let primary = meta.primary;
        let color = Plane::read(meta, primary, fetch)?;
        let alpha_item = meta.referring(*b"auxl", primary).find(|&candidate| {
            meta.properties(candidate).any(|property| {
                matches!(property, Property::Auxiliary(kind) if ALPHA_TYPES.contains(&kind.as_slice()))
            })
        });
        let alpha = match alpha_item {
            Some(item) => {
                let alpha = Plane::read(meta, item, fetch)?;
                if (alpha.width, alpha.height) != (color.width, color.height) {
                    return Err(Error::unsupported(format, "alpha of another size"));
                }
                Some(alpha)
            }
            None => None,
        };
        let premultiplied = alpha_item.is_some_and(|alpha| {
            meta.referenced(*b"prem", primary)
                .is_some_and(|to| to.contains(&alpha))
        });
        let mut profile = None;
        let mut orientation = Orientation::Upright;
        let mut crop = None;
        for property in meta.properties(primary) {
            match property {
                Property::Profile(bytes) => profile = Some(bytes.clone()),
                Property::Rotation(quarters) => {
                    orientation = orientation.then(Orientation::counter_clockwise(*quarters));
                }
                Property::Mirror(axis) => {
                    let mirror = match axis {
                        0 => Orientation::FlipVertical,
                        _ => Orientation::FlipHorizontal,
                    };
                    orientation = orientation.then(mirror);
                }
                Property::Aperture(aperture) => {
                    crop = Some(crop_of(format, aperture, color.width, color.height)?);
                }
                _ => {}
            }
        }
        Ok(Still {
            color,
            alpha,
            premultiplied,
            profile,
            orientation,
            crop,
        })
    }

    pub(crate) fn dimensions(&self) -> (u32, u32) {
        match self.crop {
            Some(crop) => (crop.width, crop.height),
            None => (self.color.width, self.color.height),
        }
    }

    pub(crate) fn layout(&self) -> Layout {
        let deep = self.color.coding.depth > 8
            || self
                .alpha
                .as_ref()
                .is_some_and(|alpha| alpha.coding.depth > 8);
        let depth = match deep {
            true => Depth::Sixteen,
            false => Depth::Eight,
        };
        Layout::new(self.color.coding.monochrome, self.alpha.is_some(), depth)
    }
}

impl Plane {
    fn read(meta: &Meta, item: u32, fetch: &mut Fetch<'_>) -> Result<Plane> {
        let format = meta.format;
        let kind = meta
            .kind(item)
            .ok_or_else(|| Error::malformed(format, "item without an entry"))?;
        match &kind {
            b"grid" => Plane::grid(meta, item, fetch),
            b"av01" | b"hvc1" | b"hev1" => {
                let (coding, width, height) = coded(meta, item, kind)?;
                Ok(Plane {
                    coding,
                    nclx: nclx_of(meta, item),
                    width,
                    height,
                    tiling: Tiling::Single(item),
                })
            }
            b"jpeg" => Err(Error::unsupported(format, "JPEG-coded items")),
            _ => Err(Error::unsupported(format, "derived or uncompressed items")),
        }
    }

    fn grid(meta: &Meta, item: u32, fetch: &mut Fetch<'_>) -> Result<Plane> {
        let format = meta.format;
        let descriptor = meta.item_head(item, fetch, GRID_DESCRIPTOR_BYTES)?;
        let mut bytes = Bytes::new(format, &descriptor);
        bytes.u8()?;
        let flags = bytes.u8()?;
        let rows = u32::from(bytes.u8()?) + 1;
        let columns = u32::from(bytes.u8()?) + 1;
        let (width, height) = match flags & 1 {
            0 => (u32::from(bytes.u16_be()?), u32::from(bytes.u16_be()?)),
            _ => (bytes.u32_be()?, bytes.u32_be()?),
        };
        let tiles = meta
            .referenced(*b"dimg", item)
            .ok_or_else(|| Error::malformed(format, "grid without tiles"))?
            .to_vec();
        let malformed = Error::malformed(format, "image grid");
        if tiles.len() as u64 != u64::from(rows) * u64::from(columns) {
            return Err(malformed);
        }
        let first = tiles[0];
        let kind = meta
            .kind(first)
            .ok_or_else(|| Error::malformed(format, "tile without an entry"))?;
        let (coding, tile_width, tile_height) = coded(meta, first, kind)?;
        for &tile in &tiles[1..] {
            let same = meta.kind(tile) == Some(kind)
                && spatial(meta, tile) == Some((tile_width, tile_height));
            if !same {
                return Err(Error::malformed(format, "image grid tiles"));
            }
        }
        let covers = |tile: u32, count: u32, side: u32| {
            u64::from(tile) * u64::from(count) >= u64::from(side)
                && u64::from(tile) * u64::from(count - 1) < u64::from(side)
        };
        if width == 0
            || height == 0
            || !covers(tile_width, columns, width)
            || !covers(tile_height, rows, height)
        {
            return Err(malformed);
        }
        Ok(Plane {
            coding,
            nclx: nclx_of(meta, item).or_else(|| nclx_of(meta, first)),
            width,
            height,
            tiling: Tiling::Grid {
                columns,
                tile_width,
                tile_height,
                tiles,
            },
        })
    }
}

fn coded(meta: &Meta, item: u32, kind: [u8; 4]) -> Result<(Coding, u32, u32)> {
    let format = meta.format;
    let config = meta
        .properties(item)
        .find_map(|property| match (property, &kind) {
            (Property::Av1Config(config), b"av01") => Some(config),
            (Property::HevcConfig(config), b"hvc1" | b"hev1") => Some(config),
            _ => None,
        });
    let config =
        config.ok_or_else(|| Error::malformed(format, "item without a codec configuration"))?;
    let (width, height) =
        spatial(meta, item).ok_or_else(|| Error::malformed(format, "item without a size"))?;
    if width == 0 || height == 0 {
        return Err(Error::InvalidSize { width, height });
    }
    Ok((Coding::new(format, kind, config)?, width, height))
}

fn spatial(meta: &Meta, item: u32) -> Option<(u32, u32)> {
    meta.properties(item).find_map(|property| match property {
        Property::Spatial { width, height } => Some((*width, *height)),
        _ => None,
    })
}

fn nclx_of(meta: &Meta, item: u32) -> Option<Nclx> {
    meta.properties(item).find_map(|property| match property {
        Property::Codes(nclx) => Some(*nclx),
        _ => None,
    })
}

fn crop_of(format: Format, aperture: &Aperture, width: u32, height: u32) -> Result<Crop> {
    let malformed = || Error::malformed(format, "clean aperture");
    let whole = |(numerator, denominator): (u32, u32)| {
        (denominator != 0)
            .then(|| numerator / denominator)
            .filter(|&side| side > 0)
    };
    let crop_width = whole(aperture.width).ok_or_else(malformed)?;
    let crop_height = whole(aperture.height).ok_or_else(malformed)?;
    let start = |(numerator, denominator): (i32, u32), side: u32, kept: u32| -> Option<u32> {
        if denominator == 0 || kept > side {
            return None;
        }
        let denominator = i64::from(denominator);
        let twice = 2 * i64::from(numerator) + (i64::from(side) - i64::from(kept)) * denominator;
        let start = twice.div_euclid(2 * denominator);
        u32::try_from(start)
            .ok()
            .filter(|&start| start.checked_add(kept).is_some_and(|end| end <= side))
    };
    Ok(Crop {
        left: start(aperture.horizontal, width, crop_width).ok_or_else(malformed)?,
        top: start(aperture.vertical, height, crop_height).ok_or_else(malformed)?,
        width: crop_width,
        height: crop_height,
    })
}
