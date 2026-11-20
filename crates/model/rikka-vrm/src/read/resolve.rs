use rikka_model_core::ImportCtx;
use rikka_model_core::MaterialId;
use rikka_model_core::MeshId;
use rikka_model_core::Model;
use rikka_model_core::NodeId;
use rikka_model_core::Warning;

pub(super) struct Resolved<'a> {
    model: &'a Model,
    meshes: &'a [Option<MeshId>],
}

impl<'a> Resolved<'a> {
    pub(super) fn new(model: &'a Model, meshes: &'a [Option<MeshId>]) -> Self {
        Self { model, meshes }
    }

    pub(super) fn model(&self) -> &'a Model {
        self.model
    }

    pub(super) fn node(&self, index: u32, what: &str, ctx: &mut ImportCtx<'_>) -> Option<NodeId> {
        match (index as usize) < self.model.nodes.len() {
            true => Some(NodeId(index)),
            false => {
                ctx.warn(Warning::Repaired(format!(
                    "skipping {what}: missing node {index}"
                )));
                None
            }
        }
    }

    pub(super) fn material(
        &self,
        index: u32,
        what: &str,
        ctx: &mut ImportCtx<'_>,
    ) -> Option<MaterialId> {
        match (index as usize) < self.model.materials.len() {
            true => Some(MaterialId(index)),
            false => {
                ctx.warn(Warning::Repaired(format!(
                    "skipping {what}: missing material {index}"
                )));
                None
            }
        }
    }

    pub(super) fn morph(
        &self,
        node: NodeId,
        index: u32,
        what: &str,
        ctx: &mut ImportCtx<'_>,
    ) -> Option<()> {
        let target_count = self
            .model
            .node(node)
            .mesh
            .map(|mesh| self.model.mesh(mesh).morph_targets.len())
            .unwrap_or(0);
        match (index as usize) < target_count {
            true => Some(()),
            false => {
                ctx.warn(Warning::Repaired(format!(
                    "skipping {what}: morph target {index} out of range for {target_count}"
                )));
                None
            }
        }
    }

    pub(super) fn mesh(&self, index: u32) -> Option<MeshId> {
        self.meshes.get(index as usize).copied().flatten()
    }

    pub(super) fn nodes_of(&self, mesh: MeshId) -> impl Iterator<Item = NodeId> + '_ {
        self.model
            .nodes
            .iter()
            .enumerate()
            .filter(move |(_, node)| node.mesh == Some(mesh))
            .map(|(index, _)| NodeId(index as u32))
    }

    pub(super) fn material_named(&self, name: &str) -> Option<MaterialId> {
        self.model
            .materials
            .iter()
            .position(|material| material.name == name)
            .map(|index| MaterialId(index as u32))
    }
}
