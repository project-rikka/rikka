use glam::{Mat3, Mat4};
use rikka_model_core::Bounds;

use crate::domain::Domain;
use crate::names::CUSTOM_NORMAL;
use crate::polymesh::Polymesh;
use crate::values::each;

const SINGULAR: f32 = 1.0e-30;

impl Polymesh {
    #[must_use]
    pub fn bounds(&self) -> Bounds {
        Bounds::from_points(self.positions.iter().copied())
    }

    pub fn transform(&mut self, matrix: &Mat4) {
        self.transform_without_flip(matrix);
        if matrix.determinant() < 0.0 {
            self.flip_faces();
        }
    }

    pub fn transform_without_flip(&mut self, matrix: &Mat4) {
        for position in &mut self.positions {
            *position = matrix.transform_point3(*position);
        }
        for key in &mut self.shape_keys.keys {
            for position in &mut key.positions {
                *position = matrix.transform_point3(*position);
            }
        }
        let linear = Mat3::from_mat4(*matrix);
        let normal_matrix = match linear.determinant().abs() > SINGULAR {
            true => linear.inverse().transpose(),
            false => Mat3::IDENTITY,
        };
        if let Some(normals) = self
            .attributes
            .values_mut::<glam::Vec3>(CUSTOM_NORMAL, Domain::Corner)
        {
            for normal in normals {
                *normal = (normal_matrix * *normal).normalize_or_zero();
            }
        }
    }

    pub fn flip_faces(&mut self) {
        let faces: Vec<std::ops::Range<usize>> =
            (0..self.face_count()).map(|face| self.face(face)).collect();
        for range in &faces {
            self.corner_verts[range.start + 1..range.end].reverse();
            self.corner_edges[range.clone()].reverse();
        }
        for attribute in self.attributes.iter_mut() {
            if attribute.domain != Domain::Corner {
                continue;
            }
            each!(&mut attribute.values, list => {
                for range in &faces {
                    list[range.start + 1..range.end].reverse();
                }
            });
        }
    }
}
