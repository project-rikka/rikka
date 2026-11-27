use rikka_model_core::glam::{Mat4, Quat, Vec2, Vec3, Vec4};

use crate::document::{
    self, Accessor, Document, Index, Shape, Sparse, UNSIGNED_BYTE, UNSIGNED_INT, UNSIGNED_SHORT,
};
use crate::read::buffers::Buffers;
use rikka_model_core::ImportCtx;
use rikka_model_core::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Component {
    I8,
    U8,
    I16,
    U16,
    U32,
    F32,
}

impl Component {
    pub(crate) fn from_code(code: u32) -> Result<Self> {
        match code {
            document::BYTE => Ok(Self::I8),
            UNSIGNED_BYTE => Ok(Self::U8),
            document::SHORT => Ok(Self::I16),
            UNSIGNED_SHORT => Ok(Self::U16),
            UNSIGNED_INT => Ok(Self::U32),
            document::FLOAT => Ok(Self::F32),
            other => Err(Error::Damaged(format!(
                "invalid accessor component type {other}"
            ))),
        }
    }

    pub(crate) fn size(self) -> usize {
        match self {
            Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::U32 | Self::F32 => 4,
        }
    }

    fn float(self, bytes: &[u8], normalized: bool) -> f32 {
        match (self, normalized) {
            (Self::F32, _) => f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            (Self::I8, true) => (f32::from(bytes[0] as i8) / 127.0).max(-1.0),
            (Self::I8, false) => f32::from(bytes[0] as i8),
            (Self::U8, true) => f32::from(bytes[0]) / 255.0,
            (Self::U8, false) => f32::from(bytes[0]),
            (Self::I16, true) => {
                (f32::from(i16::from_le_bytes([bytes[0], bytes[1]])) / 32767.0).max(-1.0)
            }
            (Self::I16, false) => f32::from(i16::from_le_bytes([bytes[0], bytes[1]])),
            (Self::U16, true) => f32::from(u16::from_le_bytes([bytes[0], bytes[1]])) / 65535.0,
            (Self::U16, false) => f32::from(u16::from_le_bytes([bytes[0], bytes[1]])),
            (Self::U32, _) => u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f32,
        }
    }

    fn integer(self, bytes: &[u8]) -> u32 {
        match self {
            Self::I8 => u32::from(bytes[0]),
            Self::U8 => u32::from(bytes[0]),
            Self::I16 | Self::U16 => u32::from(u16::from_le_bytes([bytes[0], bytes[1]])),
            Self::U32 | Self::F32 => u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Origin {
    Accessor(Index),
    SparseIndices(Index),
    SparseValues(Index),
}

impl std::fmt::Display for Origin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Accessor(index) => write!(f, "accessor {index}"),
            Self::SparseIndices(index) => write!(f, "sparse indices of accessor {index}"),
            Self::SparseValues(index) => write!(f, "sparse values of accessor {index}"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Layout {
    offset: usize,
    stride: Option<usize>,
    count: usize,
    shape: Shape,
    component: Component,
    normalized: bool,
}

#[derive(Clone, Copy, Debug)]
struct Source<'a> {
    bytes: &'a [u8],
    layout: Layout,
    stride: usize,
}

impl<'a> Source<'a> {
    fn new(bytes: &'a [u8], layout: Layout, origin: Origin) -> Result<Self> {
        let element = element_size(layout.shape, layout.component);
        let stride = layout.stride.unwrap_or(element);
        if stride < element {
            return Err(Error::Damaged(format!(
                "{origin} steps {stride} bytes between elements of {element} bytes"
            )));
        }
        let past = || Error::Damaged(format!("{origin} reads past any buffer"));
        let end = match layout.count {
            0 => layout.offset,
            count => (count - 1)
                .checked_mul(stride)
                .and_then(|span| span.checked_add(element))
                .and_then(|span| span.checked_add(layout.offset))
                .ok_or_else(past)?,
        };
        if end > bytes.len() {
            return Err(Error::Damaged(format!(
                "{origin} reads {end} bytes from a {} byte buffer view",
                bytes.len()
            )));
        }
        Ok(Self {
            bytes,
            layout,
            stride,
        })
    }

    fn at(&self, element: usize, slot: usize) -> &[u8] {
        let start = self.layout.offset
            + element * self.stride
            + component_offset(self.layout.shape, self.layout.component, slot);
        &self.bytes[start..start + self.layout.component.size()]
    }

    fn floats(&self, into: &mut [f32]) {
        let components = self.layout.shape.components();
        if self.layout.component == Component::F32 {
            let size = components * size_of::<f32>();
            for (element, values) in into
                .chunks_exact_mut(components)
                .take(self.layout.count)
                .enumerate()
            {
                let start = self.layout.offset + element * self.stride;
                let bytes = self.bytes[start..start + size].as_chunks::<4>().0;
                for (value, raw) in values.iter_mut().zip(bytes) {
                    *value = f32::from_le_bytes(*raw);
                }
            }
            return;
        }
        for element in 0..self.layout.count {
            for slot in 0..components {
                into[element * components + slot] = self
                    .layout
                    .component
                    .float(self.at(element, slot), self.layout.normalized);
            }
        }
    }

    fn integers(&self, into: &mut [u32]) {
        let components = self.layout.shape.components();
        for element in 0..self.layout.count {
            for slot in 0..components {
                into[element * components + slot] =
                    self.layout.component.integer(self.at(element, slot));
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct Accessors<'a> {
    document: &'a Document,
    buffers: &'a Buffers<'a>,
}

impl<'a> Accessors<'a> {
    pub(crate) fn new(
        document: &'a Document,
        buffers: &'a Buffers<'a>,
        ctx: &mut ImportCtx<'_>,
    ) -> Result<Self> {
        for (index, accessor) in document.accessors.iter().enumerate() {
            let values = length(accessor, index as Index)?;
            if accessor.buffer_view.is_none() {
                ctx.charge((values as u64).saturating_mul(4))?;
            }
        }
        Ok(Self { document, buffers })
    }

    pub(crate) fn count(&self, index: Index) -> Result<usize> {
        Ok(self.accessor(index)?.count)
    }

    pub(crate) fn shape(&self, index: Index) -> Result<Shape> {
        Ok(self.accessor(index)?.shape)
    }

    pub(crate) fn floats(&self, index: Index) -> Result<Vec<f32>> {
        let accessor = self.accessor(index)?;
        Component::from_code(accessor.component_type)?;
        let source = match accessor.buffer_view {
            Some(view) => Some(self.source(accessor, view, index)?),
            None => None,
        };
        let mut values = vec![0.0; length(accessor, index)?];
        if let Some(source) = source {
            source.floats(&mut values);
        }
        self.replace_sparse(accessor, index, &mut values, Source::floats)?;
        Ok(values)
    }

    pub(crate) fn integers(&self, index: Index) -> Result<Vec<u32>> {
        let accessor = self.accessor(index)?;
        if Component::from_code(accessor.component_type)? == Component::F32 {
            return Err(Error::Damaged(format!(
                "accessor {index} is floating point, expected integers"
            )));
        }
        let source = match accessor.buffer_view {
            Some(view) => Some(self.source(accessor, view, index)?),
            None => None,
        };
        let mut values = vec![0; length(accessor, index)?];
        if let Some(source) = source {
            source.integers(&mut values);
        }
        self.replace_sparse(accessor, index, &mut values, Source::integers)?;
        Ok(values)
    }

    pub(crate) fn scalars(&self, index: Index) -> Result<Vec<f32>> {
        self.shaped(index, &[Shape::Scalar])?;
        self.floats(index)
    }

    pub(crate) fn vec2(&self, index: Index) -> Result<Vec<Vec2>> {
        self.shaped(index, &[Shape::Vec2])?;
        Ok(self
            .floats(index)?
            .as_chunks::<2>()
            .0
            .iter()
            .map(|slot| Vec2::from_array(*slot))
            .collect())
    }

    pub(crate) fn vec3(&self, index: Index) -> Result<Vec<Vec3>> {
        self.shaped(index, &[Shape::Vec3])?;
        Ok(self
            .floats(index)?
            .as_chunks::<3>()
            .0
            .iter()
            .map(|slot| Vec3::from_array(*slot))
            .collect())
    }

    pub(crate) fn vec4(&self, index: Index) -> Result<Vec<Vec4>> {
        self.shaped(index, &[Shape::Vec4])?;
        Ok(self
            .floats(index)?
            .as_chunks::<4>()
            .0
            .iter()
            .map(|slot| Vec4::from_array(*slot))
            .collect())
    }

    pub(crate) fn quats(&self, index: Index) -> Result<Vec<Quat>> {
        Ok(self
            .vec4(index)?
            .into_iter()
            .map(|value| Quat::from_vec4(value).normalize())
            .collect())
    }

    pub(crate) fn matrices(&self, index: Index) -> Result<Vec<Mat4>> {
        self.shaped(index, &[Shape::Mat4])?;
        Ok(self
            .floats(index)?
            .as_chunks::<16>()
            .0
            .iter()
            .map(Mat4::from_cols_array)
            .collect())
    }

    fn shaped(&self, index: Index, allowed: &[Shape]) -> Result<()> {
        let shape = self.shape(index)?;
        match allowed.contains(&shape) {
            true => Ok(()),
            false => Err(Error::Damaged(format!(
                "accessor {index}: {shape:?} is not one of {allowed:?}"
            ))),
        }
    }

    fn accessor(&self, index: Index) -> Result<&'a Accessor> {
        self.document
            .accessors
            .get(index as usize)
            .ok_or_else(|| Error::Damaged(format!("no accessor {index}")))
    }

    fn source(&self, accessor: &Accessor, view: Index, index: Index) -> Result<Source<'a>> {
        let bytes = self.buffers.view(self.document, view)?;
        let stride = self.document.buffer_views[view as usize].byte_stride;
        Source::new(
            bytes,
            Layout {
                offset: accessor.byte_offset,
                stride,
                count: accessor.count,
                shape: accessor.shape,
                component: Component::from_code(accessor.component_type)?,
                normalized: accessor.normalized,
            },
            Origin::Accessor(index),
        )
    }

    fn replace_sparse<T: Copy + Default>(
        &self,
        accessor: &Accessor,
        index: Index,
        values: &mut [T],
        read: impl Fn(&Source<'a>, &mut [T]),
    ) -> Result<()> {
        let Some(sparse) = &accessor.sparse else {
            return Ok(());
        };
        let components = accessor.shape.components();
        let elements = self.sparse_elements(accessor, sparse, index)?;
        let source = self.sparse_values(accessor, sparse, index)?;
        let mut replacements = vec![T::default(); sparse.count * components];
        read(&source, &mut replacements);

        for (slot, element) in elements.into_iter().enumerate() {
            let from = slot * components;
            let into = element * components;
            values[into..into + components].copy_from_slice(&replacements[from..from + components]);
        }
        Ok(())
    }

    fn sparse_elements(
        &self,
        accessor: &Accessor,
        sparse: &Sparse,
        index: Index,
    ) -> Result<Vec<usize>> {
        let indices = &sparse.indices;
        let source = Source::new(
            self.buffers.view(self.document, indices.buffer_view)?,
            Layout {
                offset: indices.byte_offset,
                stride: None,
                count: sparse.count,
                shape: Shape::Scalar,
                component: Component::from_code(indices.component_type)?,
                normalized: false,
            },
            Origin::SparseIndices(index),
        )?;
        let mut elements = vec![0; sparse.count];
        source.integers(&mut elements);

        elements
            .into_iter()
            .map(|element| match (element as usize) < accessor.count {
                true => Ok(element as usize),
                false => Err(Error::Damaged(format!(
                    "accessor {index}: sparse index {element} out of range for {} elements",
                    accessor.count
                ))),
            })
            .collect()
    }

    fn sparse_values(
        &self,
        accessor: &Accessor,
        sparse: &Sparse,
        index: Index,
    ) -> Result<Source<'a>> {
        Source::new(
            self.buffers
                .view(self.document, sparse.values.buffer_view)?,
            Layout {
                offset: sparse.values.byte_offset,
                stride: None,
                count: sparse.count,
                shape: accessor.shape,
                component: Component::from_code(accessor.component_type)?,
                normalized: accessor.normalized,
            },
            Origin::SparseValues(index),
        )
    }
}

fn length(accessor: &Accessor, index: Index) -> Result<usize> {
    accessor
        .count
        .checked_mul(accessor.shape.components())
        .filter(|values| *values <= isize::MAX as usize / 16)
        .ok_or_else(|| {
            Error::Damaged(format!(
                "accessor {index} declares {} elements",
                accessor.count
            ))
        })
}
fn column_stride(shape: Shape, component: Component) -> usize {
    let rows = shape.components() / shape.columns();
    let tight = rows * component.size();
    match shape.columns() > 1 {
        true => tight.next_multiple_of(4),
        false => tight,
    }
}

fn element_size(shape: Shape, component: Component) -> usize {
    column_stride(shape, component) * shape.columns()
}

fn component_offset(shape: Shape, component: Component, slot: usize) -> usize {
    let rows = shape.components() / shape.columns();
    slot / rows * column_stride(shape, component) + slot % rows * component.size()
}
