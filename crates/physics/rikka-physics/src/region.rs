use rikka_model_core::{Model, NodeId};

pub(crate) fn triangles(model: &Model, materials: &[u32]) -> Vec<(NodeId, u32)> {
    let covers = |material: Option<u32>| {
        materials.is_empty() || material.is_some_and(|index| materials.contains(&index))
    };
    model
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| Some((NodeId(index as u32), model.mesh(node.mesh?))))
        .flat_map(|(node, mesh)| {
            mesh.submeshes
                .iter()
                .filter(|submesh| covers(submesh.material.map(|id| id.index() as u32)))
                .flat_map(move |submesh| {
                    let first = submesh.first_index / 3;
                    (first..first + submesh.index_count / 3).map(move |triangle| (node, triangle))
                })
        })
        .collect()
}
