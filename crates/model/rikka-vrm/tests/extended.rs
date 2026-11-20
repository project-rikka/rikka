use rikka_model_core::Model;
use rikka_model_core::glam::Vec3;
use rikka_vrm::{
    ColliderShape, ExtendedCollider, ExtendedColliders, ExtendedShape, Vrm, VrmExporter,
    VrmLegacyExporter,
};

use crate::support::{document, export, import, vrm};

const AVATAR: &[u8] = include_bytes!("../assets/avatar.vrm");

fn shapes() -> [ExtendedShape; 4] {
    [
        ExtendedShape::Sphere {
            offset: Vec3::new(0.0, 0.1, 0.0),
            radius: 0.5,
            inside: true,
        },
        ExtendedShape::Plane {
            offset: Vec3::new(0.0, -0.2, 0.0),
            normal: Vec3::Y,
        },
        ExtendedShape::Capsule {
            offset: Vec3::ZERO,
            radius: 0.3,
            tail: Vec3::new(0.0, 0.4, 0.0),
            inside: true,
        },
        ExtendedShape::Sphere {
            offset: Vec3::new(0.05, 0.0, 0.0),
            radius: 0.02,
            inside: false,
        },
    ]
}

fn extended_avatar() -> Model {
    let mut model = import(AVATAR.to_vec());
    let mut extended = ExtendedColliders::default();
    let spring_bone = &mut model
        .extensions
        .get_mut::<Vrm>()
        .expect("a VRM")
        .spring_bone;
    for (index, shape) in shapes().into_iter().enumerate() {
        spring_bone.colliders[index].shape = shape.fallback();
        extended.colliders.push(ExtendedCollider {
            collider: index,
            shape,
        });
    }
    model.extensions.insert(extended);
    model
}

#[test]
fn extended_colliders_round_trip() {
    let model = extended_avatar();
    let exported = export(&model, VrmExporter);

    let gltf = document(&exported.bytes);
    assert!(
        gltf["extensionsUsed"]
            .as_array()
            .expect("used extensions")
            .contains(&serde_json::json!("VRMC_springBone_extended_collider"))
    );
    let colliders = &gltf["extensions"]["VRMC_springBone"]["colliders"];
    let extension = &colliders[1]["extensions"]["VRMC_springBone_extended_collider"];
    assert_eq!(extension["specVersion"], "1.0");
    assert_eq!(
        extension["shape"]["plane"]["normal"],
        serde_json::json!([0.0, 1.0, 0.0])
    );
    assert_eq!(colliders[1]["shape"]["sphere"]["radius"], 1000.0);
    assert_eq!(
        colliders[0]["shape"]["sphere"]["offset"],
        serde_json::json!([0.0, -10000.0, 0.0])
    );
    assert!(colliders[4].get("extensions").is_none());

    let again = import(exported.bytes);
    assert_eq!(
        again.extensions.get::<ExtendedColliders>(),
        model.extensions.get::<ExtendedColliders>()
    );
    assert_eq!(vrm(&again).spring_bone, vrm(&model).spring_bone);
}

#[test]
fn extended_colliders_fall_back_in_vrm0() {
    let model = extended_avatar();
    let exported = export(&model, VrmLegacyExporter);
    assert!(
        exported
            .warnings
            .iter()
            .any(|warning| warning.to_string().contains("fallback shapes"))
    );

    let again = import(exported.bytes);
    assert!(again.extensions.get::<ExtendedColliders>().is_none());
    let radii: Vec<f32> = vrm(&again)
        .spring_bone
        .colliders
        .iter()
        .filter_map(|collider| match collider.shape {
            ColliderShape::Sphere { radius, .. } => Some(radius),
            ColliderShape::Capsule { .. } => None,
        })
        .collect();
    assert!(
        radii
            .iter()
            .any(|radius| (radius - ExtendedShape::PLANE_FALLBACK_RADIUS).abs() < 0.1)
    );
}
