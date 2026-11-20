use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use rikka_gpu::{ComputeImage, ComputeTile, ImageFormat};

use crate::error::{Error, Result};
use crate::imaging::Imaging;

const MIN_TILE_SIDE: u32 = 512;

const MAX_TILE_SIDE: u32 = 4096;

const TARGET_TILE_COUNT: u32 = 256;

const MAX_TILE_COUNT: u64 = 4096;

const SIDE_SHIFT: u32 = 16;

#[derive(Clone, Debug)]
pub(crate) struct Plane {
    tiles: Arc<[ComputeImage]>,
    width: u32,
    height: u32,
    side: u32,
    format: ImageFormat,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct PlaneRef {
    base: u32,
    across: u32,
    width: u32,
    height: u32,
}

fn side_for(width: u32, height: u32) -> u32 {
    let mut side = MIN_TILE_SIDE;
    while side < MAX_TILE_SIDE
        && u64::from(width.div_ceil(side)) * u64::from(height.div_ceil(side))
            > u64::from(TARGET_TILE_COUNT)
    {
        side *= 2;
    }
    side
}
fn allocated_tile_size(plane: [u32; 2], side: u32, [_, _, width, height]: [u32; 4]) -> [u32; 2] {
    let padded = |whole: u32, valid: u32| match whole > side && valid * 2 >= side {
        true => side,
        false => valid,
    };
    [padded(plane[0], width), padded(plane[1], height)]
}

impl Plane {
    pub(crate) fn new(
        imaging: &Imaging,
        width: u32,
        height: u32,
        format: ImageFormat,
    ) -> Result<Plane> {
        let side = Plane::tile_side(width, height)?;
        let tiles: Vec<ComputeTile<'_>> = Plane::grid(width, height, side)
            .map(|area| ComputeTile::New(allocated_tile_size([width, height], side, area)))
            .collect();
        Plane::from_tiles(imaging, &tiles, [width, height], side, format)
    }

    pub(crate) fn derived(
        imaging: &Imaging,
        source: &Plane,
        format: ImageFormat,
        is_new_tile: impl Fn(usize, [u32; 4]) -> bool,
    ) -> Result<Plane> {
        if format != source.format {
            return Plane::new(imaging, source.width, source.height, format);
        }
        let tiles: Vec<ComputeTile<'_>> = Plane::grid(source.width, source.height, source.side)
            .zip(source.tiles.iter())
            .enumerate()
            .map(|(index, (area, previous))| match is_new_tile(index, area) {
                true => ComputeTile::New(allocated_tile_size(source.extent(), source.side, area)),
                false => ComputeTile::Reuse(previous),
            })
            .collect();
        Plane::from_tiles(imaging, &tiles, source.extent(), source.side, format)
    }

    fn tile_side(width: u32, height: u32) -> Result<u32> {
        if width == 0 || height == 0 {
            return Err(Error::Invalid(format!("{width}×{height} picture is empty")));
        }
        let side = side_for(width, height);
        let count = u64::from(width.div_ceil(side)) * u64::from(height.div_ceil(side));
        if count > MAX_TILE_COUNT {
            return Err(Error::TooLarge { width, height });
        }
        Ok(side)
    }

    fn grid(width: u32, height: u32, side: u32) -> impl Iterator<Item = [u32; 4]> {
        (0..height.div_ceil(side)).flat_map(move |row| {
            (0..width.div_ceil(side)).map(move |column| {
                let [left, top] = [column * side, row * side];
                [left, top, side.min(width - left), side.min(height - top)]
            })
        })
    }

    fn from_tiles(
        imaging: &Imaging,
        tiles: &[ComputeTile<'_>],
        [width, height]: [u32; 2],
        side: u32,
        format: ImageFormat,
    ) -> Result<Plane> {
        let tiles = imaging.compute().tiles(tiles, format)?;
        Ok(Plane {
            tiles: tiles.into(),
            width,
            height,
            side,
            format,
        })
    }

    pub(crate) fn width(&self) -> u32 {
        self.width
    }

    pub(crate) fn height(&self) -> u32 {
        self.height
    }

    pub(crate) fn extent(&self) -> [u32; 2] {
        [self.width, self.height]
    }

    pub(crate) fn format(&self) -> ImageFormat {
        self.format
    }

    pub(crate) fn tiles(&self) -> &[ComputeImage] {
        &self.tiles
    }

    pub(crate) fn across(&self) -> u32 {
        self.width.div_ceil(self.side)
    }

    pub(crate) fn reference(&self) -> PlaneRef {
        PlaneRef {
            base: self.tiles[0].slot(),
            across: self.across() | self.side.trailing_zeros() << SIDE_SHIFT,
            width: self.width,
            height: self.height,
        }
    }

    pub(crate) fn tiles_with_areas(&self) -> impl Iterator<Item = ([u32; 4], &ComputeImage)> {
        Plane::grid(self.width, self.height, self.side).zip(self.tiles.iter())
    }

    pub(crate) fn areas(&self) -> impl Iterator<Item = [u32; 4]> {
        Plane::grid(self.width, self.height, self.side)
    }

    pub(crate) fn tile_aligned(&self, [left, top, width, height]: [u32; 4]) -> [u32; 4] {
        let low = [left / self.side * self.side, top / self.side * self.side];
        let high = [
            (left + width).next_multiple_of(self.side).min(self.width),
            (top + height).next_multiple_of(self.side).min(self.height),
        ];
        [low[0], low[1], high[0] - low[0], high[1] - low[1]]
    }

    pub(crate) fn shares_tile(&self, index: usize, other: &Plane) -> bool {
        self.side == other.side
            && self.extent() == other.extent()
            && self.tiles[index].aliases(&other.tiles[index])
    }

    pub(crate) fn bytes(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height) * u64::from(self.format.bytes_per_pixel())
    }
}

pub(crate) fn intersects(one: [u32; 4], other: [u32; 4]) -> bool {
    one[0] < other[0] + other[2]
        && other[0] < one[0] + one[2]
        && one[1] < other[1] + other[3]
        && other[1] < one[1] + one[3]
}

pub(crate) fn saturating_i32(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}
