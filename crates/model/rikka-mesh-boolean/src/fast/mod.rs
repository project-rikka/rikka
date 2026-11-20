mod edgenet;
mod geometry;
pub(crate) mod intersect;
pub(crate) mod separate;

use rikka_mesh_edit::{EditMesh, FaceId, IdMap};
use rikka_polymesh::{Domain, Polymesh};

use crate::boolean::{BooleanResult, FaceOrigin};
use crate::error::Result;
use crate::join::transformed;
use crate::operand::Operand;
use crate::operation::Operation;
use intersect::{IntersectOptions, intersect};

const SOURCE: &str = ".rikka_boolean_source";
const INTERSECTING: &str = ".rikka_boolean_intersecting";

fn remap_materials(mesh: &mut Polymesh, table: Option<&[i32]>) {
    let Some(table) = table else {
        return;
    };
    for material in mesh.material_indices_mut() {
        *material = match *material >= 0 && (*material as usize) < table.len() {
            true => table[*material as usize],
            false => 0,
        };
    }
}

fn tag_sources(mesh: &mut Polymesh, first: u32) {
    let count = mesh.face_count();
    let values = mesh.attributes.ensure::<i32>(SOURCE, Domain::Face, count);
    for (face, value) in values.iter_mut().enumerate() {
        *value = (first as usize + face) as i32;
    }
}

pub(crate) fn boolean(
    operands: &[Operand],
    operation: Operation,
    threshold: f32,
) -> Result<BooleanResult> {
    let negative = |operand: &Operand| operand.transform.determinant() < 0.0;
    let reference = negative(&operands[0]);
    let mut face_offsets = vec![0u32];
    for operand in operands {
        face_offsets
            .push(face_offsets.last().copied().unwrap_or(0) + operand.mesh.face_count() as u32);
    }
    let mut result = transformed(&operands[0], false, false);
    tag_sources(&mut result, 0);
    for (index, operand) in operands.iter().enumerate().skip(1) {
        let mut part = transformed(operand, false, negative(operand) != reference);
        tag_sources(&mut part, face_offsets[index]);
        remap_materials(&mut part, operand.material_remap.as_deref());
        if result.face_count() == 0 || part.face_count() == 0 {
            result = match operation {
                Operation::Intersect => Polymesh::new(),
                Operation::Union if result.face_count() == 0 => part,
                Operation::Union | Operation::Difference => result,
            };
            continue;
        }
        let first_faces = result.face_count();
        let mut joined = result;
        joined.append(&part);
        let mut mesh = EditMesh::from_polymesh(&joined)?;
        let mut sides: IdMap<FaceId, i32> = IdMap::new();
        for (position, face) in mesh.faces().enumerate() {
            sides.insert(face, i32::from(position >= first_faces));
        }
        let outcome = intersect(
            &mut mesh,
            &mut sides,
            &IntersectOptions {
                use_self: false,
                separate: false,
                dissolve: true,
                island_connect: true,
                partial_connect: false,
                edge_tag: false,
                boolean: Some(operation),
                epsilon: threshold,
            },
        )?;
        let flags = mesh.ensure_attribute::<bool>(INTERSECTING, Domain::Edge);
        for edge in &outcome.tagged_edges {
            flags[edge.index()] = true;
        }
        result = mesh.to_polymesh();
    }
    let sources: Vec<i32> = result
        .attributes
        .values::<i32>(SOURCE, Domain::Face)
        .map(<[i32]>::to_vec)
        .unwrap_or_else(|| vec![0; result.face_count()]);
    let intersecting_edges: Vec<u32> = result
        .attributes
        .values::<bool>(INTERSECTING, Domain::Edge)
        .map(|flags| {
            flags
                .iter()
                .enumerate()
                .filter(|(_, flag)| **flag)
                .map(|(edge, _)| edge as u32)
                .collect()
        })
        .unwrap_or_default();
    result.remove_attribute(SOURCE);
    result.remove_attribute(INTERSECTING);
    let face_origins = sources
        .iter()
        .map(|&source| {
            let source = source.max(0) as u32;
            let operand = (face_offsets.partition_point(|&offset| offset <= source) - 1) as u32;
            FaceOrigin {
                operand,
                face: source - face_offsets[operand as usize],
            }
        })
        .collect();
    Ok(BooleanResult {
        mesh: result,
        face_origins,
        intersecting_edges,
    })
}
