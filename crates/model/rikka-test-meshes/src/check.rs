use rikka_mesh_edit::EditMesh;
use rikka_polymesh::Polymesh;

#[must_use]
pub fn is_finite(mesh: &EditMesh) -> bool {
    mesh.verts().all(|vert| mesh.position(vert).is_finite())
        && (0..mesh.key_blocks().len()).all(|key| {
            let positions = mesh.key_positions(key);
            mesh.verts().all(|vert| positions[vert.index()].is_finite())
        })
}

pub fn check(mesh: &EditMesh, finite: bool) -> Result<(), String> {
    mesh.validate()
        .map_err(|error| format!("invalid edit mesh: {error}"))?;
    if finite && !is_finite(mesh) {
        return Err("non-finite position from finite input".to_owned());
    }
    check_polymesh(&mesh.to_polymesh(), finite)
}

pub fn check_polymesh(mesh: &Polymesh, finite: bool) -> Result<(), String> {
    mesh.validate()
        .map_err(|error| format!("invalid polymesh: {error}"))?;
    if finite {
        if !mesh.positions.iter().all(|position| position.is_finite()) {
            return Err("non-finite polymesh position from finite input".to_owned());
        }
        if !mesh
            .shape_keys
            .keys
            .iter()
            .all(|key| key.positions.iter().all(|position| position.is_finite()))
        {
            return Err("non-finite shape key position from finite input".to_owned());
        }
    }
    Ok(())
}

#[must_use]
pub fn is_polymesh_finite(mesh: &Polymesh) -> bool {
    mesh.positions.iter().all(|position| position.is_finite())
}
