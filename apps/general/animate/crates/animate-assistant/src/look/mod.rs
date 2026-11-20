mod apply;
mod describe;
mod parse;
mod request;

pub use apply::{
    FigureSlots, SlotInfo, apply_graph_edits, apply_keys, apply_materials, reset_look,
};
pub use describe::{catalog, describe_figure, describe_graph, overview};
pub(crate) use parse::{edit_graph, look, look_keys, look_lane, look_reset, materials, nodes};
pub use request::{
    Assigned, Assignment, Given, GraphEdits, GraphOp, KeyRemoval, KeySet, KeyTarget, LookKeys,
    LookQuery, LookReset, MaterialEdits, NodeRef, NodesQuery, Plug, SlotPick, SlotRef, SlotValue,
    StarterKind,
};

pub(crate) fn world_ports() -> String {
    let nodes = animate_look::world_nodes();
    animate_look::world_outputs()
        .iter()
        .map(|port| {
            let feeders: Vec<String> = nodes
                .iter()
                .filter(|spec| spec.outputs.iter().any(|output| output.ty == port.ty))
                .map(|spec| format!("`{}`", spec.id))
                .collect();
            let many = match port.many {
                true => " (many)",
                false => "",
            };
            format!("- `{}`{many} ← {}", port.name, feeders.join(", "))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn presets() -> String {
    let presets = rikka_shader_presets::presets();
    match presets.is_empty() {
        true => "none yet".to_owned(),
        false => presets
            .iter()
            .map(|preset| format!("`{}`", preset.key))
            .collect::<Vec<_>>()
            .join(", "),
    }
}
