use std::path::Path;

use rikka_model::{Input, registry};

#[test]
fn registry_covers_every_format() {
    let shipped = registry();
    assert_eq!(
        shipped.import_extensions(),
        [
            "blend", "fbx", "glb", "gltf", "pmx", "stl", "vmd", "vrm", "vrma"
        ]
    );
    assert_eq!(
        shipped.export_extensions(),
        ["blend", "fbx", "glb", "gltf", "pmx", "stl", "vmd", "vrm"]
    );

    let writer = |name: &str| {
        shipped
            .exporter_for(Path::new(name))
            .map(|exporter| exporter.info().name)
    };
    assert_eq!(writer("model.fbx"), Some("FBX"));
    assert_eq!(writer("model.glb"), Some("glTF Binary"));

    let bytes = br#"{"asset":{"version":"2.0"}}"#;
    let reader = |name: &str| {
        shipped
            .importer_for(&Input::new(bytes, Some(Path::new(name))))
            .map(|importer| importer.info().name)
    };
    assert_eq!(reader("avatar.vrm"), Some("VRM"));
    assert_eq!(reader("dance.vrma"), Some("VRM Animation"));
    assert_eq!(reader("model.gltf"), Some("glTF"));
}
