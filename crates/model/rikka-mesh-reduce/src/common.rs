use rikka_mesh_edit::{EdgeId, EditMesh, FaceId, VertId};
use rikka_polymesh::{UV_SELECT_EDGE, UV_SELECT_FACE, UV_SELECT_VERT};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Removed {
    pub verts: usize,
    pub edges: usize,
    pub faces: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Counts {
    verts: usize,
    edges: usize,
    faces: usize,
}

impl Counts {
    pub(crate) fn new(mesh: &EditMesh) -> Counts {
        Counts {
            verts: mesh.vert_count(),
            edges: mesh.edge_count(),
            faces: mesh.face_count(),
        }
    }

    pub(crate) fn removed(self, mesh: &EditMesh) -> Removed {
        Removed {
            verts: self.verts.saturating_sub(mesh.vert_count()),
            edges: self.edges.saturating_sub(mesh.edge_count()),
            faces: self.faces.saturating_sub(mesh.face_count()),
        }
    }

    pub(crate) fn changed(self, mesh: &EditMesh) -> bool {
        self.verts != mesh.vert_count()
            || self.edges != mesh.edge_count()
            || self.faces != mesh.face_count()
    }
}

pub(crate) fn selected_verts(mesh: &EditMesh) -> Vec<VertId> {
    mesh.verts()
        .filter(|&vert| mesh.is_vert_selected(vert) && !mesh.is_vert_hidden(vert))
        .collect()
}

pub(crate) fn selected_edges(mesh: &EditMesh) -> Vec<EdgeId> {
    mesh.edges()
        .filter(|&edge| mesh.is_edge_selected(edge) && !mesh.is_edge_hidden(edge))
        .collect()
}

pub(crate) fn selected_faces(mesh: &EditMesh) -> Vec<FaceId> {
    mesh.faces()
        .filter(|&face| mesh.is_face_selected(face) && !mesh.is_face_hidden(face))
        .collect()
}

pub(crate) fn any_vert_selected(mesh: &EditMesh) -> bool {
    mesh.verts().any(|vert| mesh.is_vert_selected(vert))
}

pub(crate) fn any_edge_selected(mesh: &EditMesh) -> bool {
    mesh.edges().any(|edge| mesh.is_edge_selected(edge))
}

pub(crate) fn any_face_selected(mesh: &EditMesh) -> bool {
    mesh.faces().any(|face| mesh.is_face_selected(face))
}

pub(crate) fn clear_uv_selection(mesh: &mut EditMesh) {
    for name in [UV_SELECT_VERT, UV_SELECT_EDGE, UV_SELECT_FACE] {
        mesh.remove_attribute(name);
    }
}
