use rikka_mesh_edit::{EdgeId, EditMesh, FaceId, IdSet, LoopId, VertId};

use crate::error::Result;

pub(crate) fn split_tagged_edges(mesh: &mut EditMesh, tagged: &IdSet<EdgeId>) -> Result<()> {
    let mut tagged_verts: IdSet<VertId> = IdSet::new();
    let edges: Vec<EdgeId> = mesh.edges().filter(|&edge| tagged.contains(edge)).collect();
    for &edge in &edges {
        tagged_verts.extend(mesh.edge_verts(edge));
    }
    for &edge in &edges {
        if !mesh.contains_edge(edge) {
            continue;
        }
        for vert in mesh.edge_verts(edge) {
            if !tagged_verts.remove(vert) {
                continue;
            }
            let around: Vec<EdgeId> = mesh
                .vert_edges(vert)
                .filter(|&e| tagged.contains(e))
                .collect();
            mesh.separate_vert(vert, &around, false)?;
        }
    }
    Ok(())
}

pub(crate) fn separate_faces(
    mesh: &mut EditMesh,
    filter: impl Fn(&EditMesh, FaceId) -> bool,
) -> Result<()> {
    let faces: Vec<FaceId> = mesh.faces().collect();
    let (inside, outside): (Vec<FaceId>, Vec<FaceId>) =
        faces.iter().partition(|&&face| filter(mesh, face));
    let mut unshared: IdSet<VertId> = mesh.verts().collect();
    for &face in &outside {
        for vert in mesh.face_verts_vec(face) {
            unshared.remove(vert);
        }
    }
    for &face in &inside {
        for lp in mesh.face_loops_vec(face) {
            if !mesh.contains_loop(lp) {
                continue;
            }
            let vert = mesh.loop_vert(lp);
            if unshared.contains(vert) {
                continue;
            }
            unshared.insert(vert);
            let mut split: Vec<LoopId> = Vec::new();
            for edge in mesh.vert_edges(vert).collect::<Vec<_>>() {
                for radial in mesh.edge_loops(edge).collect::<Vec<_>>() {
                    if mesh.loop_vert(radial) == vert
                        && filter(mesh, mesh.loop_face(radial))
                        && !split.contains(&radial)
                    {
                        split.push(radial);
                    }
                }
            }
            if !split.is_empty() {
                mesh.separate_loop_region_multi(&split)?;
            }
        }
    }
    Ok(())
}
