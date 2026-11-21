use rikka_parallel::Threads;
use rikka_polymesh::{Domain, MATERIAL_INDEX};

use crate::arrangement::{ArrangementInput, InputFace, Mode, Settings, arrange};
use crate::boolean::{BooleanResult, FaceOrigin};
use crate::join::join;
use crate::operand::Operand;
use crate::operation::Operation;
use crate::propagate::{Ranges, build};

pub(crate) struct ExactOptions {
    pub(crate) operation: Operation,
    pub(crate) use_self: bool,
    pub(crate) hole_tolerant: bool,
    pub(crate) threads: Threads,
}

pub(crate) fn remap_material(table: Option<&[i32]>, material: i32) -> i32 {
    match table {
        Some(table) if material >= 0 && (material as usize) < table.len() => {
            table[material as usize]
        }
        _ => material,
    }
}

pub(crate) fn boolean(operands: &[Operand], options: &ExactOptions) -> BooleanResult {
    let joined = join(operands, true);
    let ranges = Ranges {
        point_offsets: &joined.point_offsets,
        face_offsets: &joined.face_offsets,
    };
    let mesh = &joined.mesh;
    let faces: Vec<InputFace> = (0..mesh.face_count())
        .map(|face| {
            let corners = mesh.face(face);
            InputFace {
                verts: mesh.corner_verts[corners.clone()].to_vec(),
                edges: mesh.corner_edges[corners]
                    .iter()
                    .map(|&edge| Some(edge))
                    .collect(),
                shape: ranges.operand_of_face(face as u32),
            }
        })
        .collect();
    let input = ArrangementInput {
        points: mesh.positions.clone(),
        faces,
        shapes: operands.len(),
    };
    let settings = Settings {
        mode: Mode::Boolean(options.operation),
        use_self: options.use_self,
        hole_tolerant: options.hole_tolerant,
        threads: options.threads,
    };
    let arrangement = arrange(&input, settings);
    let built = build(mesh, &arrangement, &ranges);
    let mut result = built.mesh;
    let face_origins: Vec<FaceOrigin> = built
        .face_sources
        .iter()
        .map(|&face| {
            let operand = ranges.operand_of_face(face);
            FaceOrigin {
                operand,
                face: face - joined.face_offsets[operand as usize],
            }
        })
        .collect();
    if mesh.attributes.contains(MATERIAL_INDEX)
        && operands
            .iter()
            .any(|operand| operand.material_remap.is_some())
    {
        let source = mesh
            .attributes
            .values::<i32>(MATERIAL_INDEX, Domain::Face)
            .unwrap_or(&[]);
        let remapped: Vec<i32> = built
            .face_sources
            .iter()
            .zip(&face_origins)
            .map(|(&face, origin)| {
                let material = source.get(face as usize).copied().unwrap_or(0);
                remap_material(
                    operands[origin.operand as usize].material_remap.as_deref(),
                    material,
                )
            })
            .collect();
        result.material_indices_mut().copy_from_slice(&remapped);
    }
    BooleanResult {
        mesh: result,
        face_origins,
        intersecting_edges: built.intersecting_edges,
    }
}
