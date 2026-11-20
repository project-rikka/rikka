use std::fmt::Display;

use rikka_mesh_edit::{EdgeId, EditMesh, FaceId, Geometry, IdSet, VertId};

use crate::rng::Rng;

pub fn outcome<T, E: Display>(result: Result<T, E>) -> Result<(), String> {
    result.map(|_| ()).map_err(|error| error.to_string())
}

#[must_use]
pub fn selected_verts(mesh: &EditMesh) -> IdSet<VertId> {
    mesh.verts()
        .filter(|&vert| mesh.is_vert_selected(vert) && !mesh.is_vert_hidden(vert))
        .collect()
}

#[must_use]
pub fn selected_edges(mesh: &EditMesh) -> IdSet<EdgeId> {
    mesh.edges()
        .filter(|&edge| mesh.is_edge_selected(edge) && !mesh.is_edge_hidden(edge))
        .collect()
}

#[must_use]
pub fn selected_faces(mesh: &EditMesh) -> IdSet<FaceId> {
    mesh.faces()
        .filter(|&face| mesh.is_face_selected(face) && !mesh.is_face_hidden(face))
        .collect()
}

#[must_use]
pub fn selected_geometry(mesh: &EditMesh) -> Geometry {
    let mut geometry = Geometry::new();
    geometry.verts = selected_verts(mesh);
    geometry.edges = selected_edges(mesh);
    geometry.faces = selected_faces(mesh);
    geometry
}

pub fn random_vert(mesh: &EditMesh, rng: &mut Rng) -> Option<VertId> {
    let verts: Vec<VertId> = mesh.verts().collect();
    (!verts.is_empty()).then(|| verts[rng.below(verts.len())])
}

pub fn random_edge(mesh: &EditMesh, rng: &mut Rng) -> Option<EdgeId> {
    let edges: Vec<EdgeId> = mesh.edges().collect();
    (!edges.is_empty()).then(|| edges[rng.below(edges.len())])
}

pub fn random_face(mesh: &EditMesh, rng: &mut Rng) -> Option<FaceId> {
    let faces: Vec<FaceId> = mesh.faces().collect();
    (!faces.is_empty()).then(|| faces[rng.below(faces.len())])
}
