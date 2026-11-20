use glam::Mat4;
use rikka_polymesh::Polymesh;

use crate::operand::Operand;

const CLEAN_FUZZ: f32 = 1.0e-6;

pub(crate) struct Joined {
    pub(crate) mesh: Polymesh,
    pub(crate) point_offsets: Vec<u32>,
    pub(crate) face_offsets: Vec<u32>,
}

pub(crate) fn clean_transform(matrix: Mat4) -> Mat4 {
    Mat4::from_cols_array(&matrix.to_cols_array().map(|value| {
        if value.abs() <= CLEAN_FUZZ {
            0.0
        } else if (value - 1.0).abs() <= CLEAN_FUZZ {
            1.0
        } else if (value + 1.0).abs() <= CLEAN_FUZZ {
            -1.0
        } else {
            value
        }
    }))
}

pub(crate) fn transformed(operand: &Operand, clean: bool, flip: bool) -> Polymesh {
    let matrix = match clean {
        true => clean_transform(operand.transform),
        false => operand.transform,
    };
    let mut mesh = operand.mesh.clone();
    if matrix != Mat4::IDENTITY {
        mesh.transform_without_flip(&matrix);
    }
    if flip {
        mesh.flip_faces();
    }
    mesh
}

pub(crate) fn join(operands: &[Operand], clean: bool) -> Joined {
    let negative = |operand: &Operand| operand.transform.determinant() < 0.0;
    let reference = negative(&operands[0]);
    let mut mesh = Polymesh::new();
    let mut point_offsets = vec![0u32];
    let mut face_offsets = vec![0u32];
    for operand in operands {
        let part = transformed(operand, clean, negative(operand) != reference);
        mesh.append(&part);
        point_offsets.push(mesh.point_count() as u32);
        face_offsets.push(mesh.face_count() as u32);
    }
    Joined {
        mesh,
        point_offsets,
        face_offsets,
    }
}
