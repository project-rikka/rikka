use rikka_polymesh::{Polymesh, SHARP_FACE};

use super::edge_split::edge_angles;
use crate::kinds::smooth_by_angle::SmoothByAngle;

pub(crate) fn evaluate(smooth: &SmoothByAngle, mesh: &mut Polymesh) {
    mesh.ensure_edges();
    let keep_sharpness = !smooth.ignore_sharpness;
    let normals = mesh.face_normals();
    let faces_of = mesh.edge_faces();
    let angles = edge_angles(&faces_of, &normals);
    let sharp: Vec<bool> = (0..mesh.edge_count())
        .map(|edge| {
            let flat_neighbour = faces_of
                .get(edge)
                .iter()
                .any(|&face| mesh.is_sharp_face(face as usize));
            angles[edge] > smooth.angle
                || (keep_sharpness && (mesh.is_sharp_edge(edge) || flat_neighbour))
        })
        .collect();
    mesh.sharp_edges_mut().copy_from_slice(&sharp);
    mesh.attributes.remove(SHARP_FACE);
}
