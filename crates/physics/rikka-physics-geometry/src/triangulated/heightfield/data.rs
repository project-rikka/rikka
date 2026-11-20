use bytemuck::{Pod, Zeroable};
use rikka_physics_core::{Blob, BlobWriter, Section};
use rikka_physics_math::{Aabb, Vec3};

use super::view::HeightFieldView;
use crate::triangulated::triangle::Triangle;
use crate::{Error, Result};

pub const HEIGHT_FIELD_HOLE: u8 = 0xFF;
pub(crate) const BLOCK_SHIFT: u32 = 3;
pub(crate) const LEVEL_SHIFT: u32 = 27;
pub(crate) const LOCAL_MASK: u32 = (1 << LEVEL_SHIFT) - 1;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub(crate) struct Level {
    pub(crate) offset: u32,
    pub(crate) width: u32,
    pub(crate) depth: u32,
    pub(crate) shift: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct HeightFieldHeader {
    pub(crate) bounds: Aabb,
    pub(crate) scale: Vec3,
    pub(crate) min_height: f32,
    pub(crate) height_scale: f32,
    pub(crate) columns: u32,
    pub(crate) rows: u32,
    pub(crate) clockwise: u32,
    pub(crate) root: u32,
    pub(crate) heights: Section<u16>,
    pub(crate) materials: Section<u8>,
    pub(crate) flags: Section<u8>,
    pub(crate) ranges: Section<[u16; 2]>,
    pub(crate) levels: Section<Level>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeightField {
    blob: Blob<HeightFieldHeader>,
}

pub(crate) struct HeightFieldParts<'a> {
    pub(crate) header: HeightFieldHeader,
    pub(crate) heights: &'a [u16],
    pub(crate) materials: &'a [u8],
    pub(crate) flags: &'a [u8],
    pub(crate) ranges: &'a [[u16; 2]],
    pub(crate) levels: &'a [Level],
}

impl HeightField {
    pub(crate) fn assemble(parts: HeightFieldParts<'_>) -> Self {
        let mut writer = BlobWriter::<HeightFieldHeader>::new();
        let mut header = parts.header;
        header.heights = writer.push(parts.heights);
        header.materials = writer.push(parts.materials);
        header.flags = writer.push(parts.flags);
        header.ranges = writer.push(parts.ranges);
        header.levels = writer.push(parts.levels);
        Self {
            blob: writer.finish(header),
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let blob = Blob::<HeightFieldHeader>::from_bytes(bytes)?;
        let header = *blob.header();
        let corrupt = Error::CorruptShape {
            shape: "height field",
        };
        if header.columns < 2 || header.rows < 2 {
            return Err(corrupt);
        }
        let cells = (header.columns as usize - 1) * (header.rows as usize - 1);
        let heights = blob.try_section(header.heights)?;
        let materials = blob.try_section(header.materials)?;
        let flags = blob.try_section(header.flags)?;
        let ranges = blob.try_section(header.ranges)?;
        let levels = blob.try_section(header.levels)?;
        let expected = header.columns as usize * header.rows as usize;
        let levels_fit = levels.iter().all(|level| {
            (level.width as usize)
                .checked_mul(level.depth as usize)
                .and_then(|count| count.checked_add(level.offset as usize))
                .is_some_and(|end| end <= ranges.len())
                && level.shift < 32
        });
        let root_fits = levels.last().is_some_and(|level| {
            level.width == 1
                && level.depth == 1
                && header.root == (levels.len() as u32 - 1) << LEVEL_SHIFT
        });
        if heights.len() != expected
            || materials.len() != cells
            || flags.len() != 2 * cells
            || levels.len() < 2
            || !levels_fit
            || !root_fits
            || !header.scale.cmpgt(Vec3::ZERO).all()
        {
            return Err(corrupt);
        }
        Ok(Self { blob })
    }

    #[inline]
    pub fn bytes(&self) -> &[u8] {
        self.blob.bytes()
    }

    #[inline]
    pub fn content_hash(&self) -> u64 {
        self.blob.content_hash()
    }

    #[inline]
    pub(crate) fn header(&self) -> &HeightFieldHeader {
        self.blob.header()
    }

    #[inline]
    pub fn bounds(&self) -> Aabb {
        self.header().bounds
    }

    #[inline]
    pub fn scale(&self) -> Vec3 {
        self.header().scale
    }

    #[inline]
    pub fn columns(&self) -> u32 {
        self.header().columns
    }

    #[inline]
    pub fn rows(&self) -> u32 {
        self.header().rows
    }

    #[inline]
    pub fn is_clockwise(&self) -> bool {
        self.header().clockwise != 0
    }

    #[inline]
    pub fn compressed_heights(&self) -> &[u16] {
        self.blob.section(self.header().heights)
    }

    #[inline]
    pub fn materials(&self) -> &[u8] {
        self.blob.section(self.header().materials)
    }

    #[inline]
    pub fn edge_flags(&self) -> &[u8] {
        self.blob.section(self.header().flags)
    }

    #[inline]
    pub(crate) fn view(&self) -> HeightFieldView<'_> {
        let header = *self.header();
        HeightFieldView {
            header,
            heights: self.blob.section(header.heights),
            materials: self.blob.section(header.materials),
            flags: self.blob.section(header.flags),
            ranges: self.blob.section(header.ranges),
            levels: self.blob.section(header.levels),
        }
    }

    #[inline]
    pub fn triangle_count(&self) -> u32 {
        2 * (self.columns() - 1) * (self.rows() - 1)
    }

    #[inline]
    pub fn is_hole(&self, cell: u32) -> bool {
        self.materials()[cell as usize] == HEIGHT_FIELD_HOLE
    }

    #[inline]
    pub fn height(&self, column: u32, row: u32) -> f32 {
        self.view().height(column, row)
    }

    pub fn triangle(&self, index: u32) -> Triangle {
        self.view().triangle(index)
    }
}
