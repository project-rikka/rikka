use rikka_mesh_edit::{EdgeId, EditMesh, Flags, IdSet, VertId};

pub(crate) fn collapse_vert(mesh: &mut EditMesh, vert: VertId) -> Option<EdgeId> {
    let edge = mesh.vert_first_edge(vert)?;
    mesh.collapse_vert_edge(edge, vert, true).ok()
}

pub(crate) fn collapse_vert_merging(mesh: &mut EditMesh, vert: VertId) -> Option<EdgeId> {
    let (a, b) = mesh.vert_edge_pair(vert)?;
    mesh.merge_edge_flags(a, b);
    let edge = collapse_vert(mesh, vert)?;
    if !mesh.is_edge_hidden(edge) {
        let [first, second] = mesh.edge_verts(edge);
        if mesh.is_vert_hidden(first) || mesh.is_vert_hidden(second) {
            if mesh.is_edge_selected(edge) {
                mesh.select_edge_noflush(edge, false);
            }
            mesh.set_edge_flag(edge, Flags::HIDDEN, true);
        }
    }
    Some(edge)
}

pub(crate) fn split_corners(mesh: &mut EditMesh, marked: &IdSet<VertId>, delete_edges: bool) {
    let mut cleared: Vec<VertId> = Vec::new();
    let verts: Vec<VertId> = mesh.verts().collect();
    for vert in verts {
        if !marked.contains(vert) || mesh.is_vert_edge_pair(vert) {
            continue;
        }
        let corners = mesh.vert_loops(vert).collect::<Vec<_>>();
        for lp in corners {
            let face = mesh.loop_face(lp);
            if mesh.face_len(face) <= 3 {
                continue;
            }
            let next = mesh.loop_next(lp);
            let prev = mesh.loop_prev(lp);
            if !marked.contains(mesh.loop_vert(next)) && !marked.contains(mesh.loop_vert(prev)) {
                mesh.split_face(face, next, prev, None, true).ok();
            }
        }
        if delete_edges {
            cleared.push(vert);
        }
    }
    while let Some(vert) = cleared.pop() {
        while let Some(edge) = mesh.vert_first_edge(vert) {
            if mesh.kill_edge(edge).is_err() {
                break;
            }
        }
    }
}
