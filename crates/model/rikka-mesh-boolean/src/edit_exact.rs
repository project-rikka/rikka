use rikka_mesh_edit::{EdgeId, EditMesh, FaceId, FaceSample, Flags, IdMap, IdSet, VertId};

use crate::arrangement::{ArrangementInput, InputFace, Mode, Settings, arrange};
use crate::error::Result;
use crate::fast::separate::split_tagged_edges;

pub(crate) struct ExactEdit {
    pub(crate) mode: Mode,
    pub(crate) shapes: usize,
    pub(crate) use_self: bool,
    pub(crate) separate_all: bool,
    pub(crate) threads: usize,
}

pub(crate) struct EditOutcome {
    pub(crate) changed: bool,
    pub(crate) tagged_edges: Vec<EdgeId>,
}

fn attached_to_hidden_face(mesh: &EditMesh, vert: VertId) -> bool {
    mesh.vert_faces(vert).any(|face| mesh.is_face_hidden(face))
}

pub(crate) fn apply(
    mesh: &mut EditMesh,
    sides: &IdMap<FaceId, i32>,
    settings: &ExactEdit,
) -> Result<EditOutcome> {
    mesh.clear_tags(rikka_mesh_edit::ElementKind::Edge);
    let verts: Vec<VertId> = mesh.verts().collect();
    let mut vert_index: IdMap<VertId, u32> = IdMap::new();
    for (index, &vert) in verts.iter().enumerate() {
        vert_index.insert(vert, index as u32);
    }
    let edges: Vec<EdgeId> = mesh.edges().collect();
    let mut edge_index: IdMap<EdgeId, u32> = IdMap::new();
    for (index, &edge) in edges.iter().enumerate() {
        edge_index.insert(edge, index as u32);
    }
    let mut operand_faces: Vec<FaceId> = Vec::new();
    let mut faces: Vec<InputFace> = Vec::new();
    for face in mesh.faces() {
        let Some(shape) = sides.copied(face).filter(|&side| side >= 0) else {
            continue;
        };
        operand_faces.push(face);
        faces.push(InputFace {
            verts: mesh
                .face_verts(face)
                .map(|vert| vert_index.copied(vert).unwrap_or(0))
                .collect(),
            edges: mesh
                .face_edges(face)
                .map(|edge| edge_index.copied(edge))
                .collect(),
            shape: shape as u32,
        });
    }
    let input = ArrangementInput {
        points: verts.iter().map(|&vert| mesh.position(vert)).collect(),
        faces,
        shapes: settings.shapes,
    };
    let arrangement = arrange(
        &input,
        Settings {
            mode: settings.mode,
            use_self: settings.use_self,
            hole_tolerant: false,
            threads: settings.threads,
        },
    );
    let operand_set: IdSet<FaceId> = operand_faces.iter().copied().collect();
    let mut keep_verts: IdSet<VertId> = IdSet::new();
    for &vert in &verts {
        if mesh.is_vert_hidden(vert)
            || attached_to_hidden_face(mesh, vert)
            || mesh.is_vert_wire(vert)
        {
            keep_verts.insert(vert);
        }
    }
    for face in mesh
        .faces()
        .filter(|&face| !operand_set.contains(face))
        .collect::<Vec<_>>()
    {
        keep_verts.extend(mesh.face_verts(face));
    }
    let mut keep_faces: IdSet<FaceId> = IdSet::new();
    for &face in &operand_faces {
        if mesh.is_face_hidden(face) {
            keep_faces.insert(face);
        }
    }
    let mut changed = false;
    let mut output_verts: Vec<VertId> = Vec::with_capacity(arrangement.verts.len());
    for vertex in &arrangement.verts {
        let vert = match vertex.origin {
            Some(origin) => verts[origin as usize],
            None => {
                changed = true;
                mesh.add_vert(vertex.position)
            }
        };
        keep_verts.insert(vert);
        output_verts.push(vert);
    }
    let mut samples: IdMap<FaceId, FaceSample> = IdMap::new();
    for face in &arrangement.faces {
        let face_verts: Vec<VertId> = face
            .verts
            .iter()
            .map(|&v| output_verts[v as usize])
            .collect();
        if let Some(existing) = mesh.face_exists(&face_verts) {
            keep_faces.insert(existing);
            continue;
        }
        let source = operand_faces[face.face as usize];
        if !samples.contains(source) {
            samples.insert(source, mesh.sample_face(source));
        }
        let count = face_verts.len();
        for i in 0..count {
            let (a, b) = (face_verts[i], face_verts[(i + 1) % count]);
            let edge = match mesh.edge_between(a, b) {
                Some(edge) => edge,
                None => match face.edges[i] {
                    Some(original) => {
                        let example = edges[original as usize];
                        let created = mesh.add_edge_like(a, b, example)?;
                        let selected = mesh.is_edge_selected(example);
                        mesh.set_edge_flag(created, Flags::SELECT, selected);
                        created
                    }
                    None => mesh.add_edge(a, b)?,
                },
            };
            mesh.set_edge_tag(edge, face.intersect[i]);
        }
        let created = mesh
            .new_face(&face_verts)
            .existing_edges()
            .example(source)
            .create()?;
        let selected = mesh.is_face_selected(source);
        mesh.set_face_flag(created, Flags::SELECT, selected);
        if let Some(sample) = samples.get(source) {
            mesh.interpolate_face_from_sample(created, sample, false);
        }
        keep_faces.insert(created);
        changed = true;
    }
    for &face in &operand_faces {
        if !keep_faces.contains(face) && mesh.contains_face(face) {
            mesh.kill_face_loose(face)?;
            changed = true;
        }
    }
    for &vert in &verts {
        if !keep_verts.contains(vert) && mesh.contains_vert(vert) {
            mesh.kill_vert(vert)?;
            changed = true;
        }
    }
    let mut tagged: IdSet<EdgeId> = mesh
        .edges()
        .filter(|&edge| mesh.is_edge_tagged(edge))
        .collect();
    if settings.separate_all {
        split_tagged_edges(mesh, &tagged)?;
        tagged = mesh
            .edges()
            .filter(|&edge| mesh.is_edge_tagged(edge))
            .collect();
    }
    Ok(EditOutcome {
        changed,
        tagged_edges: tagged.iter().collect(),
    })
}
