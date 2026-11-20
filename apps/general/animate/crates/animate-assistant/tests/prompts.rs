use animate_look::shared_library;

const DOCUMENTS: [&str; 9] = [
    include_str!("../../../prompts/look.md"),
    include_str!("../../../prompts/system.md"),
    include_str!("../../../prompts/tools/look.md"),
    include_str!("../../../prompts/tools/nodes.md"),
    include_str!("../../../prompts/tools/edit_graph.md"),
    include_str!("../../../prompts/tools/materials.md"),
    include_str!("../../../prompts/tools/look_keys.md"),
    include_str!("../../../prompts/tools/reset_look.md"),
    include_str!("../../../prompts/tools/keys.md"),
];

#[test]
fn prompts_name_only_nodes_the_library_holds() {
    let library = shared_library();
    for document in DOCUMENTS {
        for quoted in document.split('`').skip(1).step_by(2) {
            let node_like = quoted.contains('.')
                && quoted.starts_with(|letter: char| letter.is_ascii_lowercase())
                && quoted
                    .chars()
                    .all(|letter| letter.is_ascii_lowercase() || letter == '.' || letter == '_');
            if node_like {
                assert!(
                    library.get(quoted).is_some(),
                    "a prompt names `{quoted}`, which is no node"
                );
            }
        }
    }
    let guide = DOCUMENTS[0];
    for needed in [
        "{{world_ports}}",
        "{{material_presets}}",
        "{{edit_graph_tool}}",
    ] {
        assert!(guide.contains(needed), "the guide lost {needed}");
    }
}
