pub use rikka_mesh_edit::WeldOptions;
use rikka_mesh_edit::{EditMesh, IdMap, VertId};

use crate::common::{Counts, Removed};
use crate::error::{Error, Result};

pub fn weld(
    mesh: &mut EditMesh,
    targets: &IdMap<VertId, VertId>,
    options: &WeldOptions,
) -> Result<Removed> {
    for (vert, &target) in targets.iter() {
        if vert == target
            || !mesh.contains_vert(vert)
            || !mesh.contains_vert(target)
            || targets.contains(target)
        {
            return Err(Error::InvalidWeldTarget(vert));
        }
    }
    let counts = Counts::new(mesh);
    mesh.weld_verts(targets, options);
    Ok(counts.removed(mesh))
}
