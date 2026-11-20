use rikka_mesh_edit::{EdgeId, EditMesh, FaceId};

use crate::merge::collapse_geometry;

pub fn remove_duplicate_faces(mesh: &mut EditMesh, faces: &[FaceId]) -> usize {
    let mut removed = 0;
    for &face in faces {
        if !mesh.contains_face(face) {
            continue;
        }
        if mesh.face_find_double(face).is_some() && mesh.kill_face(face).is_ok() {
            removed += 1;
        }
    }
    removed
}

pub fn collapse_short_edges(mesh: &mut EditMesh, edges: &[EdgeId], distance: f32) -> usize {
    let distance_squared = distance * distance;
    let short: Vec<EdgeId> = edges
        .iter()
        .copied()
        .filter(|&edge| {
            mesh.contains_edge(edge) && mesh.edge_length_squared(edge) < distance_squared
        })
        .collect();
    if short.is_empty() {
        return 0;
    }
    let before = mesh.vert_count();
    collapse_geometry(mesh, &short, true);
    before.saturating_sub(mesh.vert_count())
}
