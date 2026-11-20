use rikka_mesh_edit::{DeleteContext, EditMesh, Geometry};

use crate::common::{
    Counts, Removed, any_edge_selected, any_face_selected, any_vert_selected, selected_edges,
    selected_faces, selected_verts,
};
use crate::error::Result;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DeleteType {
    #[default]
    Verts,
    Edges,
    Faces,
    EdgesFaces,
    OnlyFaces,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DeleteOptions {
    pub kind: DeleteType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DeleteLooseOptions {
    pub use_verts: bool,
    pub use_edges: bool,
    pub use_faces: bool,
}

impl Default for DeleteLooseOptions {
    fn default() -> DeleteLooseOptions {
        DeleteLooseOptions {
            use_verts: true,
            use_edges: true,
            use_faces: false,
        }
    }
}

pub fn delete(mesh: &mut EditMesh, options: &DeleteOptions) -> Result<Removed> {
    let counts = Counts::new(mesh);
    let mut geometry = Geometry::new();
    let context = match options.kind {
        DeleteType::Verts => {
            geometry.verts.extend(selected_verts(mesh));
            DeleteContext::Verts
        }
        DeleteType::Edges => {
            geometry.edges.extend(selected_edges(mesh));
            DeleteContext::Edges
        }
        DeleteType::Faces => {
            geometry.faces.extend(selected_faces(mesh));
            DeleteContext::Faces
        }
        DeleteType::EdgesFaces => {
            geometry.edges.extend(selected_edges(mesh));
            geometry.faces.extend(selected_faces(mesh));
            DeleteContext::EdgesFaces
        }
        DeleteType::OnlyFaces => {
            geometry.faces.extend(selected_faces(mesh));
            DeleteContext::OnlyFaces
        }
    };
    let selected = match options.kind {
        DeleteType::Verts => any_vert_selected(mesh),
        DeleteType::Edges => any_edge_selected(mesh),
        DeleteType::Faces | DeleteType::OnlyFaces => any_face_selected(mesh),
        DeleteType::EdgesFaces => any_edge_selected(mesh) || any_face_selected(mesh),
    };
    if !selected {
        return Ok(counts.removed(mesh));
    }
    mesh.delete(context, &geometry);
    mesh.deselect_all();
    mesh.update_normals();
    Ok(counts.removed(mesh))
}

pub fn delete_loose(mesh: &mut EditMesh, options: &DeleteLooseOptions) -> Result<Removed> {
    let counts = Counts::new(mesh);
    let use_verts = options.use_verts && any_vert_selected(mesh);
    let use_edges = options.use_edges && any_edge_selected(mesh);
    let use_faces = options.use_faces && any_face_selected(mesh);
    if use_faces {
        let mut geometry = Geometry::new();
        geometry.faces = mesh
            .faces()
            .filter(|&face| {
                mesh.is_face_selected(face)
                    && mesh
                        .face_edges(face)
                        .all(|edge| mesh.is_edge_boundary(edge))
            })
            .collect();
        mesh.delete(DeleteContext::Faces, &geometry);
    }
    if use_edges {
        let mut geometry = Geometry::new();
        geometry.edges = mesh
            .edges()
            .filter(|&edge| mesh.is_edge_selected(edge) && mesh.is_edge_wire(edge))
            .collect();
        mesh.delete(DeleteContext::Edges, &geometry);
    }
    if use_verts {
        let mut geometry = Geometry::new();
        geometry.verts = mesh
            .verts()
            .filter(|&vert| mesh.is_vert_selected(vert) && mesh.vert_first_edge(vert).is_none())
            .collect();
        mesh.delete(DeleteContext::Verts, &geometry);
    }
    mesh.deselect_all();
    mesh.update_normals();
    Ok(counts.removed(mesh))
}
