mod parameters;

use std::collections::BTreeSet;

use rikka_shader_graph::{GraphKind, Library};
use rikka_shader_presets::presets;

#[test]
fn every_preset_builds_a_valid_material_under_a_unique_key() {
    let library = Library::standard();
    let mut keys = BTreeSet::new();
    for preset in presets() {
        assert!(keys.insert(preset.key), "{} is listed twice", preset.key);
        let graph = preset
            .graph(&library)
            .unwrap_or_else(|error| panic!("{}: {error}", preset.key));
        assert_eq!(graph.kind(), GraphKind::Material, "{}", preset.key);
        graph
            .material(&library, preset.key)
            .unwrap_or_else(|error| panic!("{}: {error}", preset.key));
    }
}
