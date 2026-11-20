use rikka_model_core::glam::{Vec2, Vec3, Vec4};
use rikka_model_core::{
    Export, ExportOptions, HumanBone, Import, Instance, Material, Mesh, Model, ModelBuilder,
    MorphTarget, Node, OutlineWidth, Shading, Submesh, Transform, Vertices,
};
use rikka_vrm::{ColliderShape, ExpressionPreset, Expressive, VrmVersion};
use std::sync::Arc;

use crate::support::{registry, vrm};

fn legacy() -> Model {
    let mut builder = ModelBuilder::new();
    let material = builder.add_material(Material::new("Body"));
    let mesh = builder.add_mesh(Mesh {
        vertices: Vertices {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::new(0.0, 0.0, 1.0)],
            normals: vec![Vec3::Y; 3],
            uvs: vec![Vec2::ZERO; 3],
            ..Vertices::default()
        },
        indices: vec![0, 1, 2],
        submeshes: vec![Submesh {
            name: "face".into(),
            first_index: 0,
            index_count: 3,
            material: Some(material),
            ..Submesh::default()
        }],
        morph_targets: vec![MorphTarget {
            name: "joy".into(),
            vertices: vec![1],
            positions: vec![Vec3::Y],
            normals: Vec::new(),
        }],
        ..Mesh::new("face")
    });
    let hips = builder.add_node(Node::new("hips"));
    let head = builder.add_node(
        Node::new("head")
            .mesh(mesh)
            .transform(Transform::from_translation(Vec3::new(0.0, 1.2, 0.5))),
    );
    let thumb = builder.add_node(Node::new("thumb"));
    let hair = builder.add_node(Node::new("hair"));
    let strand = builder.add_node(Node::new("strand"));
    builder.attach(hips, head);
    builder.attach(head, thumb);
    builder.attach(head, hair);
    builder.attach(hair, strand);
    builder.finish().expect("a model to hang a VRM 0.x on")
}

fn described() -> serde_json::Value {
    serde_json::json!({
        "exporterVersion": "UniVRM-0.99.0",
        "specVersion": "0.0",
        "meta": {
            "title": "Legacy",
            "author": "Someone",
            "allowedUserName": "Everyone",
            "licenseName": "CC0",
            "violentUssageName": "Allow"
        },
        "humanoid": {
            "humanBones": [
                { "bone": "hips", "node": 0 },
                { "bone": "head", "node": 1 },
                { "bone": "leftThumbProximal", "node": 2 }
            ]
        },
        "firstPerson": {
            "firstPersonBoneOffset": { "x": 0.0, "y": 0.06, "z": 0.1 },
            "lookAtTypeName": "BlendShape",
            "meshAnnotations": [{ "mesh": 0, "firstPersonFlag": "ThirdPersonOnly" }]
        },
        "blendShapeMaster": {
            "blendShapeGroups": [{
                "name": "Joy",
                "presetName": "joy",
                "binds": [{ "mesh": 0, "index": 0, "weight": 100.0 }],
                "materialValues": [{
                    "materialName": "Body",
                    "propertyName": "_Color",
                    "targetValue": [1.0, 0.0, 0.0, 1.0]
                }]
            }]
        },
        "secondaryAnimation": {
            "boneGroups": [{
                "comment": "hair",
                "stiffiness": 0.5,
                "gravityPower": 0.2,
                "gravityDir": { "x": 0.0, "y": -1.0, "z": 0.0 },
                "dragForce": 0.4,
                "hitRadius": 0.02,
                "bones": [3],
                "colliderGroups": [0]
            }],
            "colliderGroups": [{
                "node": 1,
                "colliders": [{ "offset": { "x": 0.0, "y": 0.0, "z": 0.05 }, "radius": 0.1 }]
            }]
        },
        "materialProperties": [{
            "name": "Body",
            "shader": "VRM/MToon",
            "renderQueue": 2000,
            "floatProperties": {
                "_ShadeShift": -0.3,
                "_IndirectLightIntensity": 0.3,
                "_RimFresnelPower": 3.0,
                "_RimLift": 0.1,
                "_ShadeToony": 0.9,
                "_OutlineWidthMode": 1.0,
                "_OutlineWidth": 2.0,
                "_UvAnimScrollY": 0.5,
                "_CullMode": 0.0
            },
            "vectorProperties": {
                "_Color": [1.0, 1.0, 1.0, 1.0],
                "_ShadeColor": [0.5, 0.5, 0.5, 1.0]
            },
            "textureProperties": {},
            "keywordMap": {},
            "tagMap": {}
        }]
    })
}

fn avatar() -> Model {
    let exported = Export::new(&legacy(), registry())
        .options(ExportOptions::new().embed_resources(true))
        .to_bytes("legacy.gltf")
        .expect("a glTF document");
    let mut document: serde_json::Value =
        serde_json::from_slice(&exported.bytes).expect("readable glTF");
    document["extensions"] = serde_json::json!({ "VRM": described() });
    document["extensionsUsed"] = serde_json::json!(["VRM"]);

    Import::named_bytes(
        "legacy.vrm",
        serde_json::to_vec(&document).expect("a document"),
        registry(),
    )
    .run()
    .expect("the avatar is read")
}

#[test]
fn vrm0_reads_as_vrm1() {
    let model = avatar();
    let vrm = vrm(&model);

    assert_eq!(vrm.version, VrmVersion::Legacy);
    assert_eq!(vrm.meta.name, "Legacy");
    assert_eq!(vrm.meta.authors, vec!["Someone".to_owned()]);
    assert!(vrm.meta.allow_redistribution);
    assert!(vrm.meta.allow_excessively_violent_usage);

    assert_eq!(vrm.bone(HumanBone::Hips), model.find_node("hips"));
    assert_eq!(
        vrm.bone(HumanBone::LeftThumbMetacarpal),
        model.find_node("thumb")
    );
    assert_eq!(vrm.bone(HumanBone::LeftThumbProximal), None);
}

#[test]
fn vrm0_faces_positive_z() {
    let model = avatar();
    let head = model.find_node("head").expect("a head");
    let placement = model.world_transforms()[head.index()].w_axis.truncate();

    assert!((placement - Vec3::new(0.0, 1.2, -0.5)).length() < 1e-5);
    assert_eq!(
        model.metadata.coordinates,
        rikka_model_core::CoordinateSystem::CANONICAL
    );
}

#[test]
fn vrm0_blend_shape_group_becomes_expression() {
    let model = avatar();
    let expression = vrm(&model)
        .expression(&ExpressionPreset::Happy.into())
        .expect("the happy expression");

    let bind = expression.morph_binds.first().expect("a morph bind");
    assert_eq!(bind.node, model.find_node("head").expect("a head"));
    assert_eq!(bind.index, 0);
    assert_eq!(bind.weight, 1.0);

    let tint = expression
        .material_color_binds
        .first()
        .expect("a material colour bind");
    assert_eq!(tint.value.x, 1.0);
}

#[test]
fn vrm0_spring_lists_every_joint() {
    let model = avatar();
    let spring_bone = &vrm(&model).spring_bone;
    let spring = spring_bone.springs.first().expect("a spring");

    let joints: Vec<_> = spring
        .joints
        .iter()
        .map(|joint| model.node(joint.node).name.as_str())
        .collect();
    assert_eq!(joints, vec!["hair", "strand"]);
    assert_eq!(spring.joints[0].stiffness, 0.5);
    assert_eq!(spring.joints[0].drag, 0.4);

    let collider = spring_bone.colliders.first().expect("a collider");
    assert_eq!(collider.node, model.find_node("head").expect("a head"));
    let ColliderShape::Sphere { offset, radius } = collider.shape else {
        panic!("0.x holds spheres alone");
    };
    assert!((offset - Vec3::new(0.0, 0.0, 0.05)).length() < 1e-6);
    assert_eq!(radius, 0.1);
    assert_eq!(spring_bone.colliders_of(spring).count(), 1);
}

#[test]
fn vrm0_material_becomes_toon() {
    let model = avatar();
    let material = model
        .materials
        .iter()
        .find(|material| material.name == "Body")
        .expect("the body");

    let Shading::Toon(toon) = material.shading else {
        panic!("VRM/MToon is a toon surface");
    };
    assert!(material.double_sided);
    assert_eq!(toon.shade_color, Vec3::splat(0.5_f32.powf(2.2)));
    assert_eq!(toon.outline.width, OutlineWidth::World(0.02));
    assert_eq!(toon.scroll.y, -0.5);
    let close = |a: f32, b: f32| (a - b).abs() < 1e-5;
    assert!(
        close(toon.shading_toony, 0.935) && close(toon.shading_shift, 0.235),
        "0.x toony and shift map onto the 1.0 range: {} {}",
        toon.shading_toony,
        toon.shading_shift
    );
    assert!(close(toon.gi_equalization, 0.7));
    assert!(close(toon.rim_fresnel_power, 3.0) && close(toon.rim_lift, 0.1));
}

#[test]
fn vrm0_exports_as_vrm1() {
    let model = avatar();
    let exported = Export::new(&model, registry())
        .options(ExportOptions::new().embed_resources(true))
        .to_bytes("legacy.vrm")
        .expect("the avatar is written");
    let again = Import::named_bytes("legacy.vrm", exported.bytes, registry())
        .run()
        .expect("the written avatar is read again");

    let reread = vrm(&again);
    assert_eq!(reread.version, VrmVersion::V1);
    assert_eq!(reread.meta.name, "Legacy");
    assert_eq!(
        reread.bone(HumanBone::LeftThumbMetacarpal),
        again.find_node("thumb")
    );
    assert_eq!(reread.spring_bone, vrm(&model).spring_bone);
    assert_eq!(again.materials, model.materials);
}

#[test]
fn expression_tints_and_restores_material() {
    let model = Arc::new(avatar());
    let mut instance = Instance::new(Arc::clone(&model));

    let tinted: Vec<_> = instance
        .tints()
        .expect("the avatar tints")
        .iter()
        .map(|(id, material)| (id, material.clone()))
        .collect();
    assert_eq!(tinted.len(), 1);
    let (id, rest) = tinted.into_iter().next().expect("the tinted material");
    assert_eq!(&rest, model.material(id));

    instance.set_expression(ExpressionPreset::Happy, 1.0);
    instance.refresh();
    assert_eq!(
        instance.material(id).base_color,
        Vec4::new(1.0, 0.0, 0.0, 1.0)
    );

    instance.clear_expressions();
    instance.refresh();
    assert_eq!(instance.material(id), &rest);
}
