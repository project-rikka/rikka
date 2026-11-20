use rikka_model_core::glam::{Mat4, Vec3};

use rikka_model_core::{Bounds, ByteWriter};

use crate::document::{
    ARRAY_BUFFER, Accessor, BufferView, Document, ELEMENT_ARRAY_BUFFER, FLOAT, Index, Shape,
    Sparse, SparseIndices, SparseValues, UNSIGNED_BYTE, UNSIGNED_INT, UNSIGNED_SHORT,
};

#[derive(Debug, Default)]
pub(super) struct Buffered {
    bytes: ByteWriter,
    views: Vec<BufferView>,
    accessors: Vec<Accessor>,
}

impl Buffered {
    pub(crate) fn attribute(&mut self, values: &[f32], shape: Shape) -> Index {
        let view = self.view(Some(ARRAY_BUFFER), |bytes| bytes.f32s(values));
        self.accessor(Some(view), FLOAT, values.len() / shape.components(), shape)
    }

    pub(crate) fn plain(&mut self, values: &[f32], shape: Shape) -> Index {
        let view = self.view(None, |bytes| bytes.f32s(values));
        self.accessor(Some(view), FLOAT, values.len() / shape.components(), shape)
    }

    pub(crate) fn times(&mut self, values: &[f32]) -> Index {
        let accessor = self.plain(values, Shape::Scalar);
        let accessor_entry = &mut self.accessors[accessor as usize];
        accessor_entry.min = vec![values.iter().copied().fold(f32::INFINITY, f32::min)];
        accessor_entry.max = vec![values.iter().copied().fold(f32::NEG_INFINITY, f32::max)];
        accessor
    }

    pub(crate) fn positions(&mut self, positions: &[Vec3]) -> Index {
        let accessor = self.attribute(&flatten(positions), Shape::Vec3);
        let bounds = Bounds::from_points(positions.iter().copied());
        let (least, most) = match bounds.is_empty() {
            true => (Vec3::ZERO, Vec3::ZERO),
            false => (bounds.min, bounds.max),
        };
        let accessor_entry = &mut self.accessors[accessor as usize];
        accessor_entry.min = least.to_array().to_vec();
        accessor_entry.max = most.to_array().to_vec();
        accessor
    }

    pub(crate) fn matrices(&mut self, matrices: &[Mat4]) -> Index {
        let values: Vec<f32> = matrices.iter().flat_map(Mat4::to_cols_array).collect();
        self.plain(&values, Shape::Mat4)
    }

    pub(crate) fn colors(&mut self, colors: &[[u8; 4]]) -> Index {
        let view = self.view(Some(ARRAY_BUFFER), |bytes| {
            bytes.bytes(colors.as_flattened())
        });
        let accessor = self.accessor(Some(view), UNSIGNED_BYTE, colors.len(), Shape::Vec4);
        self.accessors[accessor as usize].normalized = true;
        accessor
    }

    pub(crate) fn joints(&mut self, joints: &[[u16; 4]]) -> Index {
        let view = self.view(Some(ARRAY_BUFFER), |bytes| {
            for slot in joints.as_flattened() {
                bytes.u16(*slot);
            }
        });
        self.accessor(Some(view), UNSIGNED_SHORT, joints.len(), Shape::Vec4)
    }

    pub(crate) fn indices(&mut self, indices: &[u32]) -> Index {
        let narrow = indices.iter().copied().max().unwrap_or(0) <= u32::from(u16::MAX);
        let view = self.view(Some(ELEMENT_ARRAY_BUFFER), |bytes| {
            for &index in indices {
                match narrow {
                    true => bytes.u16(index as u16),
                    false => bytes.u32(index),
                }
            }
        });
        let component = match narrow {
            true => UNSIGNED_SHORT,
            false => UNSIGNED_INT,
        };
        self.accessor(Some(view), component, indices.len(), Shape::Scalar)
    }

    pub(crate) fn sparse(
        &mut self,
        vertices: &[u32],
        values: &[f32],
        shape: Shape,
        count: usize,
    ) -> Index {
        let accessor = self.accessor(None, FLOAT, count, shape);
        if vertices.is_empty() {
            return accessor;
        }
        let indices = self.view(None, |bytes| {
            for &vertex in vertices {
                bytes.u32(vertex);
            }
        });
        let view = self.view(None, |bytes| bytes.f32s(values));
        self.accessors[accessor as usize].sparse = Some(Sparse {
            count: vertices.len(),
            indices: SparseIndices {
                buffer_view: indices,
                byte_offset: 0,
                component_type: UNSIGNED_INT,
            },
            values: SparseValues {
                buffer_view: view,
                byte_offset: 0,
            },
        });
        accessor
    }

    pub(crate) fn raw(&mut self, bytes: &[u8]) -> Index {
        self.view(None, |buffer| buffer.bytes(bytes))
    }

    pub(crate) fn finish(self, document: &mut Document) -> Vec<u8> {
        document.buffer_views = self.views;
        document.accessors = self.accessors;
        self.bytes.finish()
    }

    fn view(&mut self, target: Option<u32>, write: impl FnOnce(&mut ByteWriter)) -> Index {
        self.bytes.align(4, 0);
        let offset = self.bytes.len();
        write(&mut self.bytes);
        self.views.push(BufferView {
            buffer: 0,
            byte_offset: offset,
            byte_length: self.bytes.len() - offset,
            byte_stride: None,
            target,
            name: None,
        });
        (self.views.len() - 1) as Index
    }

    fn accessor(
        &mut self,
        view: Option<Index>,
        component: u32,
        count: usize,
        shape: Shape,
    ) -> Index {
        self.accessors.push(Accessor {
            buffer_view: view,
            byte_offset: 0,
            component_type: component,
            normalized: false,
            count,
            shape,
            min: Vec::new(),
            max: Vec::new(),
            sparse: None,
            name: None,
        });
        (self.accessors.len() - 1) as Index
    }
}
pub(crate) fn flatten<const N: usize, T: Copy + Into<[f32; N]>>(values: &[T]) -> Vec<f32> {
    values.iter().flat_map(|value| (*value).into()).collect()
}
