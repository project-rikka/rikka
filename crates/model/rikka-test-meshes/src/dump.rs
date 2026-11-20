use std::fmt::Write as _;

use rikka_mesh_edit::{EditMesh, VertId};

pub fn python_dump(mesh: &EditMesh) -> Result<String, std::fmt::Error> {
    let verts: Vec<VertId> = mesh.verts().collect();
    let index = |vert: VertId| verts.iter().position(|&other| other == vert).unwrap_or(0);
    let mut out = String::new();
    write!(out, "V=[")?;
    for &vert in &verts {
        let p = mesh.position(vert);
        write!(out, "({:e},{:e},{:e}),", p.x, p.y, p.z)?;
    }
    write!(out, "]\nF=[")?;
    for face in mesh.faces() {
        write!(out, "(")?;
        for vert in mesh.face_verts(face) {
            write!(out, "{},", index(vert))?;
        }
        write!(out, "),")?;
    }
    write!(out, "]\nE=[")?;
    for edge in mesh.edges() {
        let [a, b] = mesh.edge_verts(edge);
        write!(out, "({},{}),", index(a), index(b))?;
    }
    write!(out, "]\nSV=[")?;
    for &vert in &verts {
        write!(out, "{},", u8::from(mesh.is_vert_selected(vert)))?;
    }
    write!(out, "]\nSE=[")?;
    for edge in mesh.edges() {
        write!(out, "{},", u8::from(mesh.is_edge_selected(edge)))?;
    }
    write!(out, "]\nSF=[")?;
    for face in mesh.faces() {
        write!(out, "{},", u8::from(mesh.is_face_selected(face)))?;
    }
    write!(out, "]\nHV=[")?;
    for &vert in &verts {
        write!(out, "{},", u8::from(mesh.is_vert_hidden(vert)))?;
    }
    write!(out, "]\nHE=[")?;
    for edge in mesh.edges() {
        write!(out, "{},", u8::from(mesh.is_edge_hidden(edge)))?;
    }
    write!(out, "]\nHF=[")?;
    for face in mesh.faces() {
        write!(out, "{},", u8::from(mesh.is_face_hidden(face)))?;
    }
    writeln!(out, "]\nMODE={:?}", mesh.select_mode().bits())?;
    Ok(out)
}
