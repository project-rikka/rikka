use crate::support::{assert_shapes, read};

#[test]
fn modifiers_match_blender() {
    let model = read("scene.blend");

    assert_eq!(model.metadata.importer, "Blender");
    assert_eq!(model.metadata.generator.as_deref(), Some("Blender 5.2"));
    assert_shapes(
        &model,
        None,
        &[
            (
                "creased",
                192,
                12.813_54,
                [-1.0, -1.0, -1.0],
                [0.8395, 0.9118, 0.9118],
            ),
            (
                "mirrored",
                1936,
                24.921_75,
                [2.6328, -0.8516, -0.9844],
                [5.3672, 0.8516, 0.9844],
            ),
            ("arrayed", 42, 84.0, [-1.0, 3.0, -1.0], [7.0, 5.0, 1.0]),
            (
                "solid",
                2019,
                33.933_82,
                [-2.5227, 3.1484, -7.0173],
                [10.5227, 11.3454, 0.9844],
            ),
            ("welded", 12, 24.0, [7.0, -1.0, -1.0], [9.0, 1.0, 1.0]),
            ("body", 44, 35.538_8, [-1.25, -7.0, 0.0], [1.25, -5.0, 4.0]),
        ],
    );
}

#[test]
fn skinning_and_shape_keys_match_blender() {
    let model = read("scene.blend");

    let animation = &model.animations[0];
    assert!((animation.duration() - 20.0 / 24.0).abs() < 1.0e-5);
    assert_shapes(
        &model,
        Some(15.0 / 24.0),
        &[(
            "body",
            44,
            37.255_49,
            [-1.4609, -8.0179, -0.0022],
            [1.4609, -4.9876, 4.1839],
        )],
    );
}

#[test]
fn reads_cameras_and_lights() {
    let model = read("scene.blend");

    assert_eq!(model.cameras.len(), 1);
    assert_eq!(model.lights.len(), 2);
    let point = model
        .lights
        .iter()
        .find(|light| matches!(light.kind, rikka_model_core::LightKind::Point { .. }))
        .expect("the point light is read");
    assert!((point.intensity - 250.0 * 683.0 / (4.0 * std::f32::consts::PI)).abs() < 1.0e-2);
}
