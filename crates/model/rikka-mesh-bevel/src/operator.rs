use rikka_mesh_edit::{EditMesh, Geometry, SelectAction, SelectMode};

use crate::bevel::bevel;
use crate::error::Result;
use crate::input::BevelInput;
use crate::options::{Affect, BevelOptions};

pub fn bevel_selected(mesh: &mut EditMesh, options: &BevelOptions) -> Result<Geometry> {
    let input = BevelInput::from_selection(mesh);
    let output = bevel(mesh, &input, options)?;
    if options.width != 0.0 {
        mesh.select_all(SelectAction::Deselect);
        for face in output.faces.iter() {
            mesh.select_face(face, true);
        }
        if options.affect == Affect::Vertices {
            for vert in output.verts.iter() {
                mesh.select_vert(vert, true);
            }
            for edge in output.edges.iter() {
                mesh.select_edge(edge, true);
            }
            if !mesh.select_mode().contains(SelectMode::VERT) {
                mesh.select_mode_flush_with(SelectMode::VERT, false);
            }
        }
    }
    if !mesh.select_mode().contains(SelectMode::FACE) {
        mesh.select_mode_flush();
    }
    Ok(output)
}
