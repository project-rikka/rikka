use rikka_mesh_edit::EditMesh;
use rikka_polymesh::Polymesh;

use crate::note::SkipReason;
use crate::origins;

pub(crate) struct Editing {
    pub(crate) mesh: EditMesh,
    active_key: Option<usize>,
}

impl Editing {
    pub(crate) fn new(source: &Polymesh) -> Result<Editing, SkipReason> {
        let mut working = source.clone();
        let active_key = working.shape_keys.active.take();
        origins::seal(&mut working);
        let mesh = EditMesh::from_polymesh(&working).map_err(|_| SkipReason::InvalidMesh)?;
        Ok(Editing { mesh, active_key })
    }

    pub(crate) fn finish(self) -> Polymesh {
        let mut result = self.mesh.to_polymesh();
        result.shape_keys.active = self
            .active_key
            .filter(|&active| active < result.shape_keys.len());
        origins::unseal(&mut result);
        result
    }
}
