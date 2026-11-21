use rikka_model_core::Endian;
use rikka_model_core::glam::{Mat4, Vec3};
use rustc_hash::FxHashSet;

use super::Blend;
use super::sdna::{Field, Layout, Scalar};

pub(crate) type Identity = (usize, u64);

#[derive(Clone, Copy)]
pub(crate) struct Record<'a> {
    blend: &'a Blend<'a>,
    owner: u32,
    layout: &'a Layout,
    bytes: &'a [u8],
}

impl std::fmt::Debug for Record<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Record")
            .field("layout", &self.layout.name)
            .field("owner", &self.owner)
            .finish_non_exhaustive()
    }
}

impl<'a> Record<'a> {
    pub(super) fn new(
        blend: &'a Blend<'a>,
        owner: u32,
        layout: &'a Layout,
        bytes: &'a [u8],
    ) -> Self {
        Self {
            blend,
            owner,
            layout,
            bytes,
        }
    }

    pub(crate) fn blend(&self) -> &'a Blend<'a> {
        self.blend
    }

    pub(crate) fn address(&self) -> Option<u64> {
        self.blend.address_of(self.bytes)
    }

    pub(crate) fn identity(&self) -> Identity {
        (
            self.blend as *const Blend<'_> as *const () as usize,
            self.address().unwrap_or(0),
        )
    }

    pub(crate) fn kind(&self) -> &'a str {
        &self.layout.name
    }

    pub(crate) fn has(&self, name: &str) -> bool {
        self.layout.field(name).is_some()
    }

    fn slot(&self, name: &str) -> Option<(&'a Field, &'a [u8])> {
        let field = self.layout.field(name)?;
        let bytes = self.bytes.get(field.offset..field.offset + field.size)?;
        Some((field, bytes))
    }

    fn scalars(&self, name: &str) -> Option<(Scalar, usize, &'a [u8])> {
        let (field, bytes) = self.slot(name)?;
        let kind = &self.blend.sdna.kinds[field.kind];
        (!field.pointer && kind.size > 0).then_some((kind.scalar, kind.size, bytes))
    }

    pub(crate) fn int(&self, name: &str) -> Option<i64> {
        let (scalar, size, bytes) = self.scalars(name)?;
        Some(integer(bytes.get(..size)?, scalar, self.blend.endian()))
    }

    pub(crate) fn float(&self, name: &str) -> Option<f32> {
        self.floats::<1>(name).map(|[value]| value)
    }

    pub(crate) fn floats<const N: usize>(&self, name: &str) -> Option<[f32; N]> {
        let (scalar, size, bytes) = self.scalars(name)?;
        let endian = self.blend.endian();
        (bytes.len() / size >= N).then(|| {
            std::array::from_fn(|slot| real(&bytes[slot * size..(slot + 1) * size], scalar, endian))
        })
    }

    pub(crate) fn vec3(&self, name: &str) -> Option<Vec3> {
        self.floats::<3>(name).map(Vec3::from_array)
    }

    pub(crate) fn matrix(&self, name: &str) -> Option<Mat4> {
        self.floats::<16>(name)
            .map(|values| Mat4::from_cols_array(&values))
    }

    pub(crate) fn text_ref(&self, name: &str) -> Option<&'a str> {
        let (field, bytes) = self.slot(name)?;
        if field.pointer {
            return None;
        }
        let end = bytes
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(bytes.len());
        std::str::from_utf8(&bytes[..end]).ok()
    }

    pub(crate) fn pointers_inline(&self, name: &str) -> Vec<u64> {
        let Some((field, bytes)) = self.slot(name) else {
            return Vec::new();
        };
        if !field.pointer {
            return Vec::new();
        }
        bytes
            .chunks_exact(self.blend.pointer_size())
            .map(|chunk| address(chunk, self.blend.endian()))
            .filter(|&address| address != 0)
            .collect()
    }

    pub(crate) fn text(&self, name: &str) -> Option<String> {
        let (field, bytes) = self.slot(name)?;
        if field.pointer {
            return None;
        }
        Some(c_string(bytes))
    }

    pub(crate) fn pointer(&self, name: &str) -> Option<u64> {
        let (field, bytes) = self.slot(name)?;
        if !field.pointer {
            return None;
        }
        let address = address(&bytes[..self.blend.pointer_size()], self.blend.endian());
        (address != 0).then_some(address)
    }

    pub(crate) fn record(&self, name: &str) -> Option<Record<'a>> {
        let (field, bytes) = self.slot(name)?;
        if field.pointer {
            return None;
        }
        let layout = self.blend.sdna.layout_of_kind(field.kind)?;
        let layout = &self.blend.sdna.layouts[layout];
        Some(Record::new(
            self.blend,
            self.owner,
            layout,
            bytes.get(..layout.size)?,
        ))
    }

    fn pointed_kind(&self, name: &str) -> Option<usize> {
        let field = self.layout.field(name)?;
        self.blend.sdna.layout_of_kind(field.kind)
    }

    fn data_block(&self, name: &str) -> Option<u32> {
        let old = self.pointer(name)?;
        self.blend.data_block(self.owner, old)
    }

    pub(crate) fn deref(&self, name: &str) -> Option<Record<'a>> {
        self.deref_all(name).into_iter().next()
    }

    pub(crate) fn deref_as(&self, name: &str, layout: &str) -> Option<Record<'a>> {
        let block = self.data_block(name)?;
        let layout = self.blend.sdna.layout(layout)?;
        self.blend.records_of(block, Some(layout)).next()
    }

    pub(crate) fn deref_all(&self, name: &str) -> Vec<Record<'a>> {
        let Some(block) = self.data_block(name) else {
            return Vec::new();
        };
        self.blend
            .records_of(block, self.pointed_kind(name))
            .collect()
    }

    pub(crate) fn table(&self, name: &str, layout: &str) -> Option<Table<'a>> {
        let block = self.data_block(name)?;
        let layout = &self.blend.sdna.layouts[self.blend.sdna.layout(layout)?];
        if layout.size == 0 || layout.field_count() == 0 {
            return None;
        }
        let bytes = self.blend.bytes_of(self.blend.block(block));
        Some(Table {
            blend: self.blend,
            owner: self.owner,
            layout,
            bytes,
        })
    }

    pub(crate) fn raw(&self, name: &str) -> Option<&'a [u8]> {
        let block = self.data_block(name)?;
        Some(self.blend.bytes_of(self.blend.block(block)))
    }

    pub(crate) fn string(&self, name: &str) -> Option<String> {
        self.raw(name).map(c_string)
    }

    pub(crate) fn pointers(&self, name: &str) -> Vec<u64> {
        let Some(bytes) = self.raw(name) else {
            return Vec::new();
        };
        let size = self.blend.pointer_size();
        bytes
            .chunks_exact(size)
            .map(|chunk| address(chunk, self.blend.endian()))
            .collect()
    }

    pub(crate) fn id(&self, name: &str) -> Option<Record<'a>> {
        self.id_at(self.pointer(name)?)
    }

    pub(crate) fn id_at(&self, old: u64) -> Option<Record<'a>> {
        self.blend.resolved(self.blend.id_block(old)?)
    }

    pub(crate) fn data_at(&self, old: u64) -> Option<Record<'a>> {
        let block = self.blend.data_block(self.owner, old)?;
        self.blend.records_of(block, None).next()
    }

    pub(crate) fn list(&self, name: &str) -> Vec<Record<'a>> {
        let Some(base) = self.record(name) else {
            return Vec::new();
        };
        let mut items = Vec::new();
        let mut seen = FxHashSet::default();
        let mut next = base.pointer("first");
        while let Some(old) = next {
            if !seen.insert(old) {
                break;
            }
            let Some(item) = self.data_at(old) else {
                break;
            };
            next = item
                .bytes
                .get(..self.blend.pointer_size())
                .map(|slot| address(slot, self.blend.endian()))
                .filter(|&address| address != 0);
            items.push(item);
        }
        items
    }

    pub(crate) fn id_name(&self) -> String {
        let name = self
            .record("id")
            .and_then(|id| id.text("name"))
            .unwrap_or_default();
        name.get(2..).unwrap_or_default().to_owned()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Table<'a> {
    blend: &'a Blend<'a>,
    owner: u32,
    layout: &'a Layout,
    bytes: &'a [u8],
}

impl<'a> Table<'a> {
    pub(crate) fn len(&self) -> usize {
        self.bytes.len() / self.layout.size
    }

    pub(crate) fn row(&self, index: usize) -> Option<Record<'a>> {
        let size = self.layout.size;
        let bytes = self.bytes.get(index * size..(index + 1) * size)?;
        Some(Record::new(self.blend, self.owner, self.layout, bytes))
    }

    pub(crate) fn rows(&self) -> impl Iterator<Item = Record<'a>> + '_ {
        (0..self.len()).filter_map(|index| self.row(index))
    }

    fn column(&self, name: &str) -> Option<(&'a Field, Scalar, usize)> {
        let field = self.layout.field(name)?;
        let kind = &self.blend.sdna.kinds[field.kind];
        (!field.pointer && kind.size > 0).then_some((field, kind.scalar, kind.size))
    }

    pub(crate) fn floats<const N: usize>(&self, name: &str) -> Option<Vec<[f32; N]>> {
        let (field, scalar, size) = self.column(name)?;
        if field.count < N {
            return None;
        }
        let endian = self.blend.endian();
        Some(
            self.bytes
                .chunks_exact(self.layout.size)
                .map(|row| {
                    std::array::from_fn(|slot| {
                        let at = field.offset + slot * size;
                        real(&row[at..at + size], scalar, endian)
                    })
                })
                .collect(),
        )
    }

    pub(crate) fn ints(&self, name: &str) -> Option<Vec<i64>> {
        let (field, scalar, size) = self.column(name)?;
        let endian = self.blend.endian();
        Some(
            self.bytes
                .chunks_exact(self.layout.size)
                .map(|row| integer(&row[field.offset..field.offset + size], scalar, endian))
                .collect(),
        )
    }
}

pub(crate) fn f32s(bytes: &[u8], endian: Endian) -> Vec<f32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| f32::from_bits(endian.u32(*chunk)))
        .collect()
}

pub(crate) fn i32s(bytes: &[u8], endian: Endian) -> Vec<i32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| endian.u32(*chunk) as i32)
        .collect()
}

pub(crate) fn i16s(bytes: &[u8], endian: Endian) -> Vec<i16> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|chunk| endian.u16(*chunk) as i16)
        .collect()
}

fn integer(bytes: &[u8], scalar: Scalar, endian: Endian) -> i64 {
    match (bytes.len(), scalar) {
        (1, Scalar::Unsigned) => i64::from(bytes[0]),
        (1, _) => i64::from(bytes[0] as i8),
        (2, Scalar::Unsigned) => i64::from(endian.u16([bytes[0], bytes[1]])),
        (2, _) => i64::from(endian.u16([bytes[0], bytes[1]]) as i16),
        (4, Scalar::Float) => real(bytes, scalar, endian) as i64,
        (4, Scalar::Unsigned) => i64::from(endian.u32(bytes.try_into().expect("four"))),
        (4, _) => i64::from(endian.u32(bytes.try_into().expect("four")) as i32),
        (8, Scalar::Float) => real(bytes, scalar, endian) as i64,
        (8, _) => endian.u64(bytes.try_into().expect("eight")) as i64,
        _ => 0,
    }
}

fn real(bytes: &[u8], scalar: Scalar, endian: Endian) -> f32 {
    match (bytes.len(), scalar) {
        (4, Scalar::Float) => f32::from_bits(endian.u32(bytes.try_into().expect("four"))),
        (8, Scalar::Float) => f64::from_bits(endian.u64(bytes.try_into().expect("eight"))) as f32,
        _ => integer(bytes, scalar, endian) as f32,
    }
}

pub(crate) fn address(bytes: &[u8], endian: Endian) -> u64 {
    match bytes.len() {
        4 => u64::from(endian.u32(bytes.try_into().expect("four"))),
        8 => endian.u64(bytes.try_into().expect("eight")),
        _ => 0,
    }
}

pub(crate) fn c_string(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}
