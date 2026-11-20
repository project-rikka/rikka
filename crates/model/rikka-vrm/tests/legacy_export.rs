use rikka_model_core::glam::{Mat3, Mat4, Quat, Vec2, Vec3};
use rikka_model_core::{
    Export, ExportOptions, HumanBone, Import, Material, Mesh, Model, ModelBuilder, MorphTarget,
    Node, Shading, Submesh, Toon, Transform, Vertices, Warning,
};
use rikka_vrm::{
    BoneLimit, ColliderShape, ExpressionName, ExpressionPreset, LegacyHumanoid, LegacyUser,
    LicenseName, LookAtKind, Vrm, VrmExporter, VrmLegacy, VrmLegacyExporter, VrmVersion,
};

use crate::schema;
use crate::support::{document, export, import, registry, vrm};

const AVATAR: &[u8] = include_bytes!("../assets/avatar.vrm");

const CLOSE: f32 = 1e-4;

fn sample() -> Model {
    import(AVATAR.to_vec())
}

fn legacy(model: &Model) -> &VrmLegacy {
    model.extensions.get::<VrmLegacy>().expect("0.x data")
}

fn triangle(name: &str, material: rikka_model_core::MaterialId, morphs: &[&str]) -> Mesh {
    Mesh {
        vertices: Vertices {
            positions: vec![
                Vec3::ZERO,
                Vec3::new(0.1, 0.0, 0.0),
                Vec3::new(0.0, 0.1, 0.05),
            ],
            normals: vec![Vec3::Z; 3],
            uvs: vec![Vec2::ZERO, Vec2::X, Vec2::Y],
            ..Vertices::default()
        },
        indices: vec![0, 1, 2],
        submeshes: vec![Submesh {
            name: name.into(),
            first_index: 0,
            index_count: 3,
            material: Some(material),
            ..Submesh::default()
        }],
        morph_targets: morphs
            .iter()
            .map(|morph| MorphTarget {
                name: (*morph).into(),
                vertices: vec![1],
                positions: vec![Vec3::Y * 0.01],
                normals: Vec::new(),
            })
            .collect(),
        ..Mesh::new(name)
    }
}

fn skeleton() -> Model {
    let mut builder = ModelBuilder::new();
    let body = builder.add_material(Material::new("Body"));
    let hair_material = builder.add_material(Material::new("Hair"));
    let cloth_material = builder.add_material(Material::new("Cloth"));
    let cloth = builder.add_mesh(triangle("cloth", cloth_material, &[]));
    let face = builder.add_mesh(triangle("face", body, &["joy", "a"]));
    let hair_mesh = builder.add_mesh(triangle("hair", hair_material, &[]));
    let at = |x: f32, y: f32, z: f32| Transform::from_translation(Vec3::new(x, y, z));

    let hips = builder.add_node(Node::new("hips").mesh(cloth).transform(at(0.0, 1.0, 0.0)));
    let spine = builder.add_node(Node::new("spine").transform(at(0.0, 0.1, 0.0)));
    let chest = builder.add_node(Node::new("chest").transform(at(0.0, 0.1, 0.0)));
    let neck = builder.add_node(Node::new("neck").transform(at(0.0, 0.2, 0.0)));
    let head = builder.add_node(Node::new("head").mesh(face).transform(at(0.0, 0.1, 0.0)));
    let hair = builder.add_node(
        Node::new("hair")
            .mesh(hair_mesh)
            .transform(at(0.0, 0.1, -0.05)),
    );
    let strand = builder.add_node(Node::new("strand").transform(at(0.0, -0.1, 0.0)));
    let thumb = builder.add_node(Node::new("thumb").transform(at(0.1, 0.0, 0.0)));
    let tail = builder.add_node(Node::new("tail").transform(at(0.0, 0.0, -0.1)));
    let tail_end = builder.add_node(Node::new("tail_end").transform(at(0.0, 0.0, -0.1)));
    builder.attach(hips, spine);
    builder.attach(spine, chest);
    builder.attach(chest, neck);
    builder.attach(neck, head);
    builder.attach(head, hair);
    builder.attach(hair, strand);
    builder.attach(head, thumb);
    builder.attach(hips, tail);
    builder.attach(tail, tail_end);
    builder.finish().expect("a skeleton to hang a VRM 0.x on")
}

fn described() -> serde_json::Value {
    let linear = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 0.0];
    serde_json::json!({
        "exporterVersion": "UniVRM-0.99.0",
        "specVersion": "0.0",
        "meta": {
            "title": "Rich",
            "version": "2.1",
            "author": "Someone",
            "contactInformation": "https://example.com/contact",
            "reference": "https://example.com/original",
            "allowedUserName": "ExplicitlyLicensedPerson",
            "violentUssageName": "Disallow",
            "sexualUssageName": "Allow",
            "commercialUssageName": "Allow",
            "otherPermissionUrl": "https://example.com/permission",
            "licenseName": "Other",
            "otherLicenseUrl": "https://example.com/license"
        },
        "humanoid": {
            "humanBones": [
                { "bone": "hips", "node": 0, "useDefaultValues": true },
                { "bone": "spine", "node": 1 },
                { "bone": "chest", "node": 2 },
                { "bone": "neck", "node": 3 },
                {
                    "bone": "head",
                    "node": 4,
                    "useDefaultValues": false,
                    "min": { "x": -10.0, "y": -20.0, "z": -30.0 },
                    "max": { "x": 10.0, "y": 20.0, "z": 30.0 },
                    "center": { "x": 0.0, "y": 1.0, "z": 0.0 },
                    "axisLength": 0.25
                },
                { "bone": "leftThumbProximal", "node": 7 }
            ],
            "armStretch": 0.1,
            "legStretch": 0.2,
            "upperArmTwist": 0.3,
            "lowerArmTwist": 0.4,
            "upperLegTwist": 0.6,
            "lowerLegTwist": 0.7,
            "feetSpacing": 0.05,
            "hasTranslationDoF": true
        },
        "firstPerson": {
            "firstPersonBone": 3,
            "firstPersonBoneOffset": { "x": 0.01, "y": 0.06, "z": 0.1 },
            "meshAnnotations": [
                { "mesh": 1, "firstPersonFlag": "ThirdPersonOnly" },
                { "mesh": 2, "firstPersonFlag": "Both" }
            ],
            "lookAtTypeName": "BlendShape",
            "lookAtHorizontalInner": { "curve": linear, "xRange": 80.0, "yRange": 1.0 },
            "lookAtHorizontalOuter": {
                "curve": [0.0, 0.0, 0.0, 2.0, 1.0, 1.0, 0.5, 0.0],
                "xRange": 70.0,
                "yRange": 0.75
            },
            "lookAtVerticalDown": { "curve": linear, "xRange": 60.0, "yRange": 0.5 },
            "lookAtVerticalUp": { "curve": linear, "xRange": 50.0, "yRange": 0.25 }
        },
        "blendShapeMaster": {
            "blendShapeGroups": [
                {
                    "name": "Smile",
                    "presetName": "joy",
                    "binds": [{ "mesh": 1, "index": 0, "weight": 50.0 }],
                    "materialValues": [
                        {
                            "materialName": "Body",
                            "propertyName": "_Color",
                            "targetValue": [1.0, 0.0, 0.0, 1.0]
                        },
                        {
                            "materialName": "Body",
                            "propertyName": "_MainTex_ST",
                            "targetValue": [0.5, 0.5, 0.25, 0.25]
                        },
                        {
                            "materialName": "Body",
                            "propertyName": "_OutlineWidth",
                            "targetValue": [0.25]
                        }
                    ],
                    "isBinary": false
                },
                {
                    "name": "A",
                    "presetName": "a",
                    "binds": [{ "mesh": 1, "index": 1, "weight": 100.0 }],
                    "isBinary": true
                },
                {
                    "name": "Wink",
                    "presetName": "unknown",
                    "binds": [{ "mesh": 1, "index": 0, "weight": 25.0 }]
                }
            ]
        },
        "secondaryAnimation": {
            "boneGroups": [{
                "comment": "hair",
                "stiffiness": 0.5,
                "gravityPower": 0.25,
                "gravityDir": { "x": 0.0, "y": -1.0, "z": 0.0 },
                "dragForce": 0.375,
                "center": -1,
                "hitRadius": 0.02,
                "bones": [5, 8],
                "colliderGroups": [0, 1]
            }],
            "colliderGroups": [
                {
                    "node": 4,
                    "colliders": [
                        { "offset": { "x": 0.0, "y": 0.0, "z": 0.05 }, "radius": 0.1 },
                        { "offset": { "x": 0.02, "y": 0.1, "z": 0.0 }, "radius": 0.05 }
                    ]
                },
                {
                    "node": 0,
                    "colliders": [{ "offset": { "x": 0.0, "y": 0.1, "z": 0.0 }, "radius": 0.2 }]
                }
            ]
        },
        "materialProperties": [
            {
                "name": "Body",
                "shader": "VRM/MToon",
                "renderQueue": 2450,
                "floatProperties": {
                    "_BlendMode": 1.0,
                    "_Cutoff": 0.3,
                    "_ShadeShift": -0.3,
                    "_ShadeToony": 0.9,
                    "_IndirectLightIntensity": 0.3,
                    "_RimFresnelPower": 3.0,
                    "_RimLift": 0.1,
                    "_RimLightingMix": 0.5,
                    "_OutlineWidthMode": 2.0,
                    "_OutlineWidth": 0.6,
                    "_OutlineColorMode": 1.0,
                    "_OutlineLightingMix": 0.75,
                    "_UvAnimScrollX": 0.25,
                    "_UvAnimScrollY": 0.5,
                    "_UvAnimRotation": 0.125,
                    "_CullMode": 0.0,
                    "_BumpScale": 0.8
                },
                "vectorProperties": {
                    "_Color": [0.9, 0.8, 0.7, 1.0],
                    "_ShadeColor": [0.5, 0.4, 0.3, 1.0],
                    "_RimColor": [0.2, 0.2, 0.2, 1.0],
                    "_EmissionColor": [0.1, 0.2, 0.3, 1.0],
                    "_OutlineColor": [0.1, 0.1, 0.1, 1.0],
                    "_MainTex": [0.1, 0.2, 0.5, 0.5]
                },
                "textureProperties": {},
                "keywordMap": { "_ALPHATEST_ON": true },
                "tagMap": { "RenderType": "TransparentCutout" }
            },
            {
                "name": "Hair",
                "shader": "VRM/MToon",
                "renderQueue": 2501,
                "floatProperties": { "_BlendMode": 3.0, "_ZWrite": 1.0, "_ShadeToony": 0.5 },
                "vectorProperties": { "_Color": [1.0, 1.0, 1.0, 0.5] },
                "textureProperties": {},
                "keywordMap": { "_ALPHABLEND_ON": true },
                "tagMap": { "RenderType": "Transparent" }
            },
            {
                "name": "Cloth",
                "shader": "VRM_USE_GLTFSHADER",
                "renderQueue": -1,
                "floatProperties": {},
                "vectorProperties": {},
                "textureProperties": {},
                "keywordMap": {},
                "tagMap": {}
            }
        ]
    })
}

fn legacy_avatar() -> Model {
    let exported = Export::new(&skeleton(), registry())
        .options(ExportOptions::new().embed_resources(true))
        .to_bytes("legacy.gltf")
        .expect("a glTF document");
    let mut gltf: serde_json::Value =
        serde_json::from_slice(&exported.bytes).expect("readable glTF");
    gltf["extensions"] = serde_json::json!({ "VRM": described() });
    gltf["extensionsUsed"] = serde_json::json!(["VRM"]);
    Import::named_bytes(
        "legacy.vrm",
        serde_json::to_vec(&gltf).expect("a document"),
        registry(),
    )
    .run()
    .expect("the 0.x avatar is read")
}

fn close(one: f32, other: f32) -> bool {
    (one - other).abs() <= CLOSE * one.abs().max(other.abs()).max(1.0)
}

fn close_vec(one: Vec3, other: Vec3) -> bool {
    (0..3).all(|axis| close(one[axis], other[axis]))
}

fn assert_materials_close(before: &[Material], after: &[Material]) {
    assert_eq!(before.len(), after.len());
    for (one, other) in before.iter().zip(after) {
        let name = &one.name;
        assert_eq!(one.name, other.name);
        assert_eq!(one.alpha, other.alpha, "{name}: alpha");
        assert_eq!(one.double_sided, other.double_sided, "{name}: culling");
        assert!(
            (0..4).all(|channel| close(one.base_color[channel], other.base_color[channel])),
            "{name}: base colour {} {}",
            one.base_color,
            other.base_color
        );
        assert!(close_vec(one.emissive, other.emissive), "{name}: emission");
        assert!(
            close(one.uv_transform.offset.x, other.uv_transform.offset.x)
                && close(one.uv_transform.offset.y, other.uv_transform.offset.y)
                && close(one.uv_transform.scale.x, other.uv_transform.scale.x)
                && close(one.uv_transform.scale.y, other.uv_transform.scale.y),
            "{name}: texture placement"
        );
        let (one_textures, other_textures) = (&one.textures, &other.textures);
        assert_eq!(one_textures.base_color, other_textures.base_color, "{name}");
        assert_eq!(one_textures.normal, other_textures.normal, "{name}");
        assert_eq!(one_textures.emissive, other_textures.emissive, "{name}");
        assert_eq!(one_textures.shade, other_textures.shade, "{name}");
        assert_eq!(one_textures.matcap, other_textures.matcap, "{name}");
        assert_eq!(one_textures.rim, other_textures.rim, "{name}");
        assert_eq!(
            one_textures.outline_width, other_textures.outline_width,
            "{name}"
        );
        assert_eq!(
            one_textures.uv_animation_mask, other_textures.uv_animation_mask,
            "{name}"
        );
        match (one.shading, other.shading) {
            (Shading::Toon(one_toon), Shading::Toon(other_toon)) => {
                assert_toon_close(name, &one_toon, &other_toon, one_textures.matcap.is_some());
            }
            (Shading::Pbr(one_pbr), Shading::Pbr(other_pbr)) => {
                assert_eq!(one_pbr, other_pbr, "{name}: surface");
            }
            _ => panic!("{name}: shading model changed"),
        }
    }
}

fn assert_toon_close(name: &str, one: &Toon, other: &Toon, matcap: bool) {
    let pairs = [
        ("shading shift", one.shading_shift, other.shading_shift),
        ("shading toony", one.shading_toony, other.shading_toony),
        (
            "gi equalization",
            one.gi_equalization,
            other.gi_equalization,
        ),
        ("normal scale", one.normal_scale, other.normal_scale),
        (
            "rim fresnel",
            one.rim_fresnel_power,
            other.rim_fresnel_power,
        ),
        ("rim lift", one.rim_lift, other.rim_lift),
        ("rim mix", one.rim_lighting_mix, other.rim_lighting_mix),
        (
            "outline width",
            one.outline.width.width(),
            other.outline.width.width(),
        ),
        (
            "outline mix",
            one.outline.lighting_mix,
            other.outline.lighting_mix,
        ),
        ("scroll x", one.scroll.x, other.scroll.x),
        ("scroll y", one.scroll.y, other.scroll.y),
        ("spin", one.spin, other.spin),
    ];
    for (what, one_value, other_value) in pairs {
        assert!(
            close(one_value, other_value),
            "{name}: {what} {one_value} {other_value}"
        );
    }
    assert!(
        close_vec(one.shade_color, other.shade_color),
        "{name}: shade"
    );
    assert!(close_vec(one.rim_color, other.rim_color), "{name}: rim");
    assert!(
        close_vec(one.outline.color, other.outline.color),
        "{name}: outline colour"
    );
    if matcap {
        assert!(
            close_vec(one.matcap_color, other.matcap_color),
            "{name}: matcap"
        );
    }
    assert_eq!(
        std::mem::discriminant(&one.outline.width),
        std::mem::discriminant(&other.outline.width),
        "{name}: outline mode"
    );
    assert_eq!(
        one.transparent_depth_write, other.transparent_depth_write,
        "{name}"
    );
    assert_eq!(one.render_order, other.render_order, "{name}: render order");
}

fn placed_vertices(model: &Model) -> Vec<Vec3> {
    let world = model.world_transforms();
    let mut placed = Vec::new();
    for (index, node) in model.nodes.iter().enumerate() {
        let Some(mesh) = node.mesh else {
            continue;
        };
        let vertices = &model.mesh(mesh).vertices;
        match node.skin {
            Some(skin) => {
                let skin = model.skin(skin);
                let joints: Vec<Mat4> = skin
                    .joints
                    .iter()
                    .zip(&skin.inverse_bind)
                    .map(|(joint, bind)| world[joint.index()] * *bind)
                    .collect();
                for (vertex, position) in vertices.positions.iter().enumerate() {
                    let influences = vertices.joints[vertex];
                    let weights = vertices.weights[vertex];
                    let skinned: Vec3 = (0..4)
                        .map(|slot| {
                            joints[influences[slot] as usize].transform_point3(*position)
                                * weights[slot]
                        })
                        .sum();
                    placed.push(skinned);
                }
            }
            None => placed.extend(
                vertices
                    .positions
                    .iter()
                    .map(|position| world[index].transform_point3(*position)),
            ),
        }
    }
    placed
}

fn world_point(model: &Model, node: rikka_model_core::NodeId, offset: Vec3) -> Vec3 {
    model.world_transforms()[node.index()].transform_point3(offset)
}

fn has_warning(warnings: &[Warning], fragment: &str) -> bool {
    warnings
        .iter()
        .any(|warning| warning.to_string().contains(fragment))
}

#[test]
fn vrm0_keeps_legacy_fields() {
    let model = legacy_avatar();
    let kept = legacy(&model);

    let license = &kept.license;
    assert_eq!(license.allowed_user, LegacyUser::ExplicitlyLicensedPerson);
    assert!(!license.violent_usage && license.sexual_usage && license.commercial_usage);
    assert_eq!(license.license, LicenseName::Other);
    assert_eq!(
        license.other_permission_url.as_deref(),
        Some("https://example.com/permission")
    );
    assert_eq!(
        license.other_license_url.as_deref(),
        Some("https://example.com/license")
    );

    assert_eq!(
        kept.humanoid,
        LegacyHumanoid {
            arm_stretch: 0.1,
            leg_stretch: 0.2,
            upper_arm_twist: 0.3,
            lower_arm_twist: 0.4,
            upper_leg_twist: 0.6,
            lower_leg_twist: 0.7,
            feet_spacing: 0.05,
            has_translation_dof: true,
        }
    );
    assert_eq!(kept.bone_limits.len(), 1);
    assert_eq!(
        kept.bone_limits.get(&HumanBone::Head),
        Some(&BoneLimit {
            use_default_values: false,
            min: Vec3::new(-10.0, -20.0, -30.0),
            max: Vec3::new(10.0, 20.0, 30.0),
            center: Vec3::Y,
            axis_length: 0.25,
        })
    );
    assert_eq!(kept.first_person_bone, model.find_node("neck"));
    assert_eq!(
        kept.look_at_curves.horizontal_outer,
        vec![0.0, 0.0, 0.0, 2.0, 1.0, 1.0, 0.5, 0.0]
    );

    let smile = kept
        .expression(&ExpressionPreset::Happy.into())
        .expect("the joy group");
    assert_eq!(smile.group_name, "Smile");
    let value = smile.material_values.first().expect("a 0.x-only value");
    assert_eq!(value.property, "_OutlineWidth");
    assert_eq!(value.target_value, vec![0.25]);
    assert!(
        vrm(&model)
            .expression(&ExpressionName::Custom("Wink".to_owned()))
            .is_some()
    );
}

#[test]
fn vrm0_round_trips_every_field() {
    let model = legacy_avatar();
    let exported = export(&model, VrmLegacyExporter);
    let again = import(exported.bytes);

    let (before, after) = (vrm(&model), vrm(&again));
    assert_eq!(after.version, VrmVersion::Legacy);
    assert_eq!(before.meta, after.meta);
    assert_eq!(before.humanoid, after.humanoid);
    assert_eq!(before.first_person, after.first_person);
    assert_eq!(before.look_at.kind, LookAtKind::Expression);
    assert_eq!(before.look_at, after.look_at);
    assert_eq!(before.expressions, after.expressions);
    assert_eq!(before.spring_bone, after.spring_bone);
    assert_eq!(legacy(&model), legacy(&again));
    assert_materials_close(&model.materials, &again.materials);
    assert_eq!(placed_vertices(&model), placed_vertices(&again));
}

#[test]
fn sample_avatar_writes_as_vrm0() {
    let model = sample();
    let exported = export(&model, VrmLegacyExporter);
    assert!(has_warning(&exported.warnings, "baked"));
    assert!(has_warning(&exported.warnings, "node constraints"));
    assert!(has_warning(&exported.warnings, "sphere chains"));
    let again = import(exported.bytes);

    let (before, after) = (vrm(&model), vrm(&again));
    assert_eq!(after.version, VrmVersion::Legacy);
    assert_eq!(before.meta.name, after.meta.name);
    assert_eq!(before.meta.version, after.meta.version);
    assert_eq!(before.meta.authors, after.meta.authors);
    assert_eq!(before.meta.avatar_permission, after.meta.avatar_permission);
    assert_eq!(
        before.meta.allow_redistribution,
        after.meta.allow_redistribution
    );
    assert!(after.meta.thumbnail.is_some());
    assert_eq!(before.humanoid, after.humanoid);
    assert_eq!(before.first_person, after.first_person);
    assert!(after.constraints.is_empty());

    assert_eq!(before.expressions.len(), after.expressions.len());
    for expression in &before.expressions {
        let written = after
            .expression(&expression.name)
            .unwrap_or_else(|| panic!("expression {} is kept", expression.name));
        assert_eq!(expression.morph_binds.len(), written.morph_binds.len());
        for (one, other) in expression.morph_binds.iter().zip(&written.morph_binds) {
            assert_eq!((one.node, one.index), (other.node, other.index));
            assert!(close(one.weight, other.weight));
        }
        assert_eq!(
            expression.material_color_binds,
            written.material_color_binds
        );
        assert_eq!(expression.is_binary, written.is_binary);
    }

    let head = before.bone(HumanBone::Head).expect("a head");
    assert!(close_vec(
        world_point(&model, head, before.look_at.offset_from_head),
        world_point(&again, head, after.look_at.offset_from_head)
    ));
    assert_eq!(before.look_at.kind, after.look_at.kind);

    let (springs, written_springs) = (&before.spring_bone.springs, &after.spring_bone.springs);
    assert_eq!(springs.len(), written_springs.len());
    for (spring, written) in springs.iter().zip(written_springs) {
        let nodes = |spring: &rikka_vrm::Spring| {
            spring
                .joints
                .iter()
                .map(|joint| joint.node)
                .collect::<Vec<_>>()
        };
        assert_eq!(nodes(spring), nodes(written));
        let (head_joint, written_joint) = (spring.joints[0], written.joints[0]);
        assert!(close(head_joint.stiffness, written_joint.stiffness));
        assert!(close(head_joint.drag, written_joint.drag));
        assert!(close(head_joint.hit_radius, written_joint.hit_radius));
        assert!(close_vec(
            head_joint.gravity_direction,
            written_joint.gravity_direction
        ));
    }

    let written_colliders = &after.spring_bone.colliders;
    assert!(
        written_colliders
            .iter()
            .all(|collider| matches!(collider.shape, ColliderShape::Sphere { .. }))
    );
    let covered = |node, point: Vec3, radius: f32| {
        written_colliders.iter().any(|collider| {
            let ColliderShape::Sphere {
                offset,
                radius: written_radius,
            } = collider.shape
            else {
                return false;
            };
            collider.node == node
                && close(radius, written_radius)
                && close_vec(point, world_point(&again, node, offset))
        })
    };
    for collider in &before.spring_bone.colliders {
        match collider.shape {
            ColliderShape::Sphere { offset, radius } => {
                assert!(covered(
                    collider.node,
                    world_point(&model, collider.node, offset),
                    radius
                ));
            }
            ColliderShape::Capsule {
                offset,
                radius,
                tail,
            } => {
                assert!(covered(
                    collider.node,
                    world_point(&model, collider.node, offset),
                    radius
                ));
                assert!(covered(
                    collider.node,
                    world_point(&model, collider.node, tail),
                    radius
                ));
            }
        }
    }

    let (placed, written_placed) = (placed_vertices(&model), placed_vertices(&again));
    assert_eq!(placed.len(), written_placed.len());
    assert!(
        placed
            .iter()
            .zip(&written_placed)
            .all(|(one, other)| close_vec(*one, *other)),
        "the rest pose is kept while nodes lose rotation and scale"
    );
    assert!(again.nodes.iter().all(|node| {
        node.transform.rotation == Quat::IDENTITY && node.transform.scale == Vec3::ONE
    }));
    assert_materials_close(&model.materials, &again.materials);
}

#[test]
fn vrm1_survives_vrm0_and_back() {
    let original = sample();
    let legacy_model = import(export(&original, VrmLegacyExporter).bytes);
    let again = import(export(&legacy_model, VrmExporter).bytes);

    let (through, back) = (vrm(&legacy_model), vrm(&again));
    assert_eq!(back.version, VrmVersion::V1);
    assert_eq!(through.meta, back.meta);
    assert_eq!(through.humanoid, back.humanoid);
    assert_eq!(through.first_person, back.first_person);
    assert_eq!(through.look_at, back.look_at);
    assert_eq!(through.expressions, back.expressions);
    assert_eq!(through.spring_bone, back.spring_bone);
    assert_eq!(legacy_model.materials, again.materials);

    let first = vrm(&original);
    assert_eq!(first.humanoid, back.humanoid);
    assert_eq!(first.first_person, back.first_person);
    assert_eq!(first.meta.name, back.meta.name);
    assert_eq!(first.expressions.len(), back.expressions.len());
}

#[test]
fn vrm0_export_follows_schema() {
    for model in [sample(), legacy_avatar()] {
        let exported = export(&model, VrmLegacyExporter);
        let gltf = document(&exported.bytes);
        let described = &gltf["extensions"]["VRM"];
        let found = schema::violations(described, &schema::vrm0());
        assert!(found.is_empty(), "{found:#?}");

        let used: Vec<&str> = gltf["extensionsUsed"]
            .as_array()
            .expect("used extensions")
            .iter()
            .filter_map(|name| name.as_str())
            .collect();
        assert!(used.contains(&"VRM"));
        assert!(!used.iter().any(|name| name.starts_with("VRMC_")));

        let count = |key: &str| gltf[key].as_array().map_or(0, Vec::len) as u64;
        let in_range = |value: &serde_json::Value, key: &str| {
            value.as_u64().is_some_and(|index| index < count(key))
        };
        assert_eq!(
            described["materialProperties"]
                .as_array()
                .expect("material properties")
                .len() as u64,
            count("materials")
        );
        for bone in described["humanoid"]["humanBones"]
            .as_array()
            .expect("bones")
        {
            assert!(in_range(&bone["node"], "nodes"));
        }
        assert!(in_range(&described["meta"]["texture"], "textures") || model.textures.is_empty());
        assert!(in_range(
            &described["firstPerson"]["firstPersonBone"],
            "nodes"
        ));
        for annotation in described["firstPerson"]["meshAnnotations"]
            .as_array()
            .expect("annotations")
        {
            assert!(in_range(&annotation["mesh"], "meshes"));
        }
        let material_names: Vec<&str> = gltf["materials"]
            .as_array()
            .expect("materials")
            .iter()
            .filter_map(|material| material["name"].as_str())
            .collect();
        for group in described["blendShapeMaster"]["blendShapeGroups"]
            .as_array()
            .expect("groups")
        {
            for bind in group["binds"].as_array().expect("binds") {
                assert!(in_range(&bind["mesh"], "meshes"));
            }
            for value in group["materialValues"].as_array().expect("values") {
                let name = value["materialName"].as_str().expect("a material name");
                assert_eq!(
                    material_names
                        .iter()
                        .filter(|known| **known == name)
                        .count(),
                    1
                );
            }
        }
        let secondary = &described["secondaryAnimation"];
        let groups = secondary["colliderGroups"].as_array().expect("groups");
        for group in groups {
            assert!(in_range(&group["node"], "nodes"));
        }
        for bone_group in secondary["boneGroups"].as_array().expect("bone groups") {
            for bone in bone_group["bones"].as_array().expect("bones") {
                assert!(in_range(bone, "nodes"));
            }
            for group in bone_group["colliderGroups"].as_array().expect("groups") {
                assert!(
                    group
                        .as_u64()
                        .is_some_and(|group| group < groups.len() as u64)
                );
            }
        }
        for node in gltf["nodes"].as_array().expect("nodes") {
            assert!(node.get("rotation").is_none() && node.get("scale").is_none());
            assert!(node.get("matrix").is_none());
        }
    }
}

#[test]
fn schema_check_reports_violations() {
    let exported = export(&legacy_avatar(), VrmLegacyExporter);
    let mut described = document(&exported.bytes)["extensions"]["VRM"].clone();
    described["meta"]["licenseName"] = serde_json::json!("MIT");
    described["blendShapeMaster"]["blendShapeGroups"][0]["binds"][0]["weight"] =
        serde_json::json!(150.0);
    described["humanoid"]["humanBones"][0]["node"] = serde_json::json!("hips");

    assert_eq!(schema::violations(&described, &schema::vrm0()).len(), 3);
}

#[test]
fn vrm0_export_refuses_missing_nodes() {
    let mut model = skeleton();
    let mut avatar = Vrm::default();
    avatar
        .humanoid
        .set_bone(HumanBone::Head, Some(rikka_model_core::NodeId(99)));
    model.extensions.insert(avatar);

    let refused = Export::with_exporter(&model, std::sync::Arc::new(VrmLegacyExporter))
        .to_bytes("avatar.vrm")
        .expect_err("a dangling node is refused");
    assert!(matches!(refused, rikka_model_core::Error::Invalid(_)));
}

#[test]
fn vrm0_export_bakes_rotated_nodes() {
    let mut builder = ModelBuilder::new();
    let material = builder.add_material(Material::new("Skin"));
    let shared = builder.add_mesh(triangle("shared", material, &["pout"]));
    let mirrored = builder.add_mesh(triangle("mirrored", material, &[]));
    let root = builder.add_node(Node::new("root").transform(Transform {
        translation: Vec3::new(0.0, 1.0, 0.0),
        rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        scale: Vec3::splat(2.0),
    }));
    let tilted = builder.add_node(Node::new("tilted").mesh(shared).transform(Transform {
        translation: Vec3::new(0.1, 0.0, 0.0),
        rotation: Quat::from_rotation_x(0.7),
        scale: Vec3::ONE,
    }));
    let rolled = builder.add_node(Node::new("rolled").mesh(shared).transform(Transform {
        translation: Vec3::new(-0.1, 0.0, 0.0),
        rotation: Quat::from_rotation_z(-0.4),
        scale: Vec3::ONE,
    }));
    let flipped = builder.add_node(Node::new("flipped").mesh(mirrored).transform(Transform {
        translation: Vec3::new(0.0, 0.2, 0.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::new(-1.0, 1.0, 1.0),
    }));
    builder.attach(root, tilted);
    builder.attach(root, rolled);
    builder.attach(root, flipped);
    let mut model = builder.finish().expect("a tilted model");
    model.extensions.insert(Vrm::default());

    let exported = export(&model, VrmLegacyExporter);
    assert!(has_warning(&exported.warnings, "baked 4 nodes"));
    let again = import(exported.bytes);

    let (placed, written) = (placed_vertices(&model), placed_vertices(&again));
    assert_eq!(placed.len(), written.len());
    assert!(
        placed
            .iter()
            .zip(&written)
            .all(|(one, other)| close_vec(*one, *other))
    );

    let before_world = model.world_transforms();
    let after_world = again.world_transforms();
    for node in [tilted, rolled, flipped] {
        let before_mesh = model.mesh(model.node(node).mesh.expect("a mesh"));
        let after_mesh = again.mesh(again.node(node).mesh.expect("a mesh"));
        let normal_matrix = Mat3::from_mat4(before_world[node.index()])
            .inverse()
            .transpose();
        let expected = (normal_matrix * before_mesh.vertices.normals[0]).normalize();
        let found = (Mat3::from_mat4(after_world[node.index()]) * after_mesh.vertices.normals[0])
            .normalize();
        assert!(close_vec(expected, found), "{expected} {found}");

        let front = |mesh: &Mesh, world: Mat4| {
            let corner = |index: usize| {
                world.transform_point3(mesh.vertices.positions[mesh.indices[index] as usize])
            };
            (corner(1) - corner(0)).cross(corner(2) - corner(0)) * world.determinant().signum()
        };
        assert!(
            front(before_mesh, before_world[node.index()])
                .dot(front(after_mesh, after_world[node.index()]))
                > 0.0
        );

        if let (Some(before_target), Some(after_target)) = (
            before_mesh.morph_targets.first(),
            after_mesh.morph_targets.first(),
        ) {
            let moved = before_world[node.index()].transform_vector3(before_target.positions[0]);
            let written_moved =
                after_world[node.index()].transform_vector3(after_target.positions[0]);
            assert!(close_vec(moved, written_moved));
        }
    }
}
