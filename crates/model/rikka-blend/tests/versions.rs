use rikka_model_core::Warning;

use crate::support::{assert_shapes, read};

#[test]
fn linked_collections_are_instanced() {
    let model = read("linking.blend");

    assert!(
        !model
            .warnings
            .iter()
            .any(|warning| matches!(warning, Warning::MissingResource { .. })),
        "{:?}",
        model.warnings
    );
    assert_shapes(
        &model,
        None,
        &[(
            "ball",
            80,
            11.665_93,
            [0.0489, 2.0, -1.0],
            [1.9511, 4.0, 1.0],
        )],
    );
}

#[test]
fn reads_gzipped_blender_2_5x_32_bit() {
    let model = read("TorusLightsCams_250_compressed.blend");

    assert_eq!(model.metadata.generator.as_deref(), Some("Blender 2.52"));
    assert_shapes(
        &model,
        None,
        &[(
            "Torus",
            4608,
            9.300_88,
            [-1.2341, -1.2341, -0.2379],
            [1.2341, 1.2341, 0.2379],
        )],
    );
    assert_eq!(model.cameras.len(), 2);
    assert_eq!(model.lights.len(), 5);
}
