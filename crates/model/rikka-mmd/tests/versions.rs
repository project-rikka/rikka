use std::sync::Arc;

use rikka_mmd::{
    MMD_UNIT_SCALE, Pmx, Pmx21Exporter, PmxAeroModel, PmxAnchor, PmxBone, PmxBoneFlags,
    PmxClusterConfig, PmxDisplayFrame, PmxDisplayTarget, PmxExporter, PmxGroupOffset, PmxImporter,
    PmxImpulseOffset, PmxJoint, PmxJointKind, PmxLimits, PmxMaterial, PmxMaterialFlags, PmxMorph,
    PmxOffsets, PmxPanel, PmxPhysics, PmxRigidBody, PmxShape, PmxSoftBody, PmxSoftBodyConfig,
    PmxSoftBodyFlags, PmxSoftBodyIterations, PmxSoftBodyMaterial, PmxSoftBodyShape, PmxUvOffset,
    PmxVersion, PmxVertices, PmxWeight,
};
use rikka_model_core::glam::{Mat4, Quat, Vec2, Vec3, Vec4};
use rikka_model_core::{
    CoordinateSystem, Error, Export, ExportOptions, Exporter, Import, ImportOptions,
    MemoryResolver, MemorySink, Mesh, Model, ModelBuilder, MorphTarget, Node, NodeId, Registry,
    Result, Sink, Skin, Submesh, Transform, Vertices,
};

const MIKU: &[u8] = include_bytes!("../assets/mmd/miku.pmx");
const BONES: usize = 4;
const VERTICES: usize = 4;
const SLOT: usize = 2;
const UV_CHANNELS: u8 = 4;

type Edit = fn(&mut Pmx);

fn registry() -> Arc<Registry> {
    Arc::new(Registry::new().with_importer(PmxImporter))
}

fn native(bytes: Vec<u8>) -> Result<Model> {
    Import::named_bytes("model.pmx", bytes, registry())
        .resolver(Arc::new(MemoryResolver::new()))
        .options(
            ImportOptions::new()
                .coordinates(CoordinateSystem::LEFT_HANDED_Y_UP)
                .normalize_units(false)
                .normalize_weights(false),
        )
        .run()
}

fn export(model: &Model, exporter: impl Exporter + 'static) -> Result<Vec<u8>> {
    let sink = Arc::new(MemorySink::new());
    Export::with_exporter(model, Arc::new(exporter))
        .options(
            ExportOptions::new()
                .coordinates(CoordinateSystem::LEFT_HANDED_Y_UP)
                .unit_scale(MMD_UNIT_SCALE),
        )
        .sink(sink as Arc<dyn Sink>)
        .to_bytes("model.pmx")
        .map(|exported| exported.bytes)
}

fn version_of(bytes: &[u8]) -> f32 {
    f32::from_le_bytes(bytes[4..8].try_into().expect("four version bytes"))
}

fn pmx(model: &Model) -> &Pmx {
    model.extensions.get::<Pmx>().expect("the document")
}

fn morph(name: &str, offsets: PmxOffsets) -> PmxMorph {
    PmxMorph {
        name: name.to_owned(),
        english_name: name.to_owned(),
        panel: PmxPanel::Other,
        offsets,
    }
}

fn rigid_body(name: &str, bone: usize) -> PmxRigidBody {
    PmxRigidBody {
        name: name.to_owned(),
        english_name: String::new(),
        bone: Some(bone),
        group: 1,
        collision_mask: 0xfffe,
        shape: PmxShape::Capsule,
        size: Vec3::new(0.5, 1.0, 0.0),
        position: Vec3::new(0.0, 1.0, 0.0),
        rotation: Quat::IDENTITY,
        mass: 1.0,
        linear_damping: 0.5,
        angular_damping: 0.5,
        restitution: 0.0,
        friction: 0.5,
        physics: PmxPhysics::Dynamic,
    }
}

fn soft_body() -> PmxSoftBody {
    PmxSoftBody {
        name: "skirt".to_owned(),
        english_name: "Skirt".to_owned(),
        shape: PmxSoftBodyShape::Rope,
        material: Some(0),
        group: 3,
        collision_mask: 0x00ff,
        flags: PmxSoftBodyFlags(
            PmxSoftBodyFlags::BENDING_LINKS
                | PmxSoftBodyFlags::CLUSTERS
                | PmxSoftBodyFlags::LINK_CROSSING,
        ),
        bending_distance: 2,
        cluster_count: 5,
        total_mass: 1.5,
        collision_margin: 0.25,
        aero_model: PmxAeroModel::FaceOneSided,
        config: PmxSoftBodyConfig {
            velocity_correction: 1.0,
            damping: 0.125,
            drag: 0.25,
            lift: 0.375,
            pressure: 0.5,
            volume_conservation: 0.625,
            dynamic_friction: 0.75,
            pose_matching: 0.875,
            rigid_contact_hardness: 1.0,
            kinetic_contact_hardness: 0.1,
            soft_contact_hardness: 0.2,
            anchor_hardness: 0.3,
        },
        cluster: PmxClusterConfig {
            soft_rigid_hardness: 0.1,
            soft_kinetic_hardness: 1.0,
            soft_soft_hardness: 0.5,
            soft_rigid_impulse_split: 0.4,
            soft_kinetic_impulse_split: 0.6,
            soft_soft_impulse_split: 0.7,
        },
        iterations: PmxSoftBodyIterations {
            velocity: 1,
            position: 2,
            drift: 3,
            cluster: 4,
        },
        material_stiffness: PmxSoftBodyMaterial {
            linear_stiffness: 1.0,
            area_stiffness: 0.5,
            volume_stiffness: 0.25,
        },
        anchors: vec![PmxAnchor {
            body: 1,
            vertex: 3,
            near_mode: true,
        }],
        pins: vec![2],
    }
}

fn document(nodes: &[NodeId]) -> Pmx {
    let bones = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| PmxBone {
            name: format!("bone{index}"),
            english_name: format!("Bone{index}"),
            node: *node,
            position: Vec3::new(0.0, index as f32, 0.0),
            parent: index.checked_sub(1),
            flags: PmxBoneFlags(
                PmxBoneFlags::ROTATABLE | PmxBoneFlags::VISIBLE | PmxBoneFlags::OPERABLE,
            ),
            ..PmxBone::default()
        })
        .collect();
    let uv_morph = |channel: u8| PmxOffsets::Uv {
        channel,
        offsets: vec![PmxUvOffset {
            vertex: u32::from(channel) % VERTICES as u32,
            offset: Vec4::new(0.5, 0.25, 0.0, 1.0),
        }],
    };
    Pmx {
        name: "synthetic".to_owned(),
        english_name: "Synthetic".to_owned(),
        comment: "comment".to_owned(),
        english_comment: "Comment".to_owned(),
        textures: vec!["tex/skin.png".to_owned()],
        vertices: PmxVertices {
            weights: vec![
                PmxWeight::One,
                PmxWeight::Two,
                PmxWeight::One,
                PmxWeight::Four,
            ],
            additional_uvs: (1..=4)
                .map(|set| {
                    (0..VERTICES)
                        .map(|vertex| Vec4::splat((set * 10 + vertex) as f32 / 64.0))
                        .collect()
                })
                .collect(),
            spherical: Vec::new(),
            edge_scales: vec![1.0, 0.5, 0.25, 2.0],
        },
        materials: vec![PmxMaterial {
            name: "skin".to_owned(),
            english_name: "Skin".to_owned(),
            diffuse: Vec4::new(1.0, 0.5, 0.25, 1.0),
            specular: Vec3::splat(0.125),
            specular_power: 5.0,
            ambient: Vec3::splat(0.5),
            flags: PmxMaterialFlags(PmxMaterialFlags::DOUBLE_SIDED | PmxMaterialFlags::OUTLINE),
            edge_color: Vec4::new(0.0, 0.0, 0.0, 1.0),
            edge_size: 1.0,
            texture: Some(0),
            ..PmxMaterial::default()
        }],
        bones,
        morphs: vec![
            morph("a", PmxOffsets::Vertex),
            morph(
                "g",
                PmxOffsets::Group(vec![PmxGroupOffset {
                    morph: 0,
                    ratio: 0.5,
                }]),
            ),
            morph("slot", PmxOffsets::Group(Vec::new())),
            morph("uv0", uv_morph(0)),
            morph("uv1", uv_morph(1)),
            morph("uv2", uv_morph(2)),
            morph("uv3", uv_morph(3)),
            morph("uv4", uv_morph(4)),
        ],
        display_frames: vec![
            PmxDisplayFrame {
                name: "Root".to_owned(),
                english_name: "Root".to_owned(),
                special: true,
                targets: vec![PmxDisplayTarget::Bone(0)],
            },
            PmxDisplayFrame {
                name: "表情".to_owned(),
                english_name: "Exp".to_owned(),
                special: true,
                targets: vec![PmxDisplayTarget::Morph(0), PmxDisplayTarget::Morph(1)],
            },
        ],
        rigid_bodies: vec![rigid_body("upper", 1), rigid_body("lower", 2)],
        joints: vec![PmxJoint {
            name: "joint".to_owned(),
            english_name: "Joint".to_owned(),
            kind: PmxJointKind::Spring6Dof.byte(),
            bodies: (Some(0), Some(1)),
            position: Vec3::new(0.0, 1.5, 0.0),
            rotation: Quat::IDENTITY,
            translation_limits: PmxLimits {
                lower: Vec3::splat(-0.25),
                upper: Vec3::splat(0.25),
            },
            rotation_limits: PmxLimits {
                lower: Vec3::splat(-0.5),
                upper: Vec3::splat(0.5),
            },
            translation_spring: Vec3::splat(10.0),
            rotation_spring: Vec3::splat(20.0),
        }],
        ..Pmx::default()
    }
}

fn synthetic(edit: impl FnOnce(&mut Pmx)) -> Model {
    let mut vertices = Vertices {
        positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::new(1.0, 1.0, 0.0)],
        normals: vec![Vec3::Z; VERTICES],
        uvs: vec![
            Vec2::ZERO,
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, 1.0),
            Vec2::ONE,
        ],
        ..Vertices::default()
    };
    vertices.set_influences(0, &[(0, 1.0)]);
    vertices.set_influences(1, &[(0, 0.25), (1, 0.75)]);
    vertices.set_influences(2, &[(2, 1.0)]);
    vertices.set_influences(3, &[(0, 0.5), (1, 0.25), (2, 0.125), (3, 0.125)]);
    let mut moved = MorphTarget::new("a");
    moved.push(1, Vec3::new(0.0, 0.5, 0.0), None);
    let mut targets = vec![moved];
    targets.extend(["g", "slot"].map(MorphTarget::new));
    targets.extend((0..=UV_CHANNELS).map(|channel| MorphTarget::new(format!("uv{channel}"))));

    let mut builder = ModelBuilder::new()
        .coordinates(CoordinateSystem::LEFT_HANDED_Y_UP)
        .unit_scale(MMD_UNIT_SCALE);
    let morphs = targets.len();
    let mesh = builder.add_mesh(Mesh {
        vertices,
        indices: vec![0, 1, 2, 1, 3, 2],
        submeshes: vec![Submesh {
            name: "skin".to_owned(),
            first_index: 0,
            index_count: 6,
            ..Submesh::default()
        }],
        morph_targets: targets,
        ..Mesh::new("synthetic")
    });
    let root = builder.add_node(
        Node::new("synthetic")
            .mesh(mesh)
            .morph_weights(vec![0.0; morphs]),
    );
    let nodes: Vec<NodeId> = (0..BONES)
        .map(|index| {
            builder.add_node(
                Node::new(format!("bone{index}"))
                    .transform(Transform::from_translation(Vec3::new(0.0, 1.0, 0.0))),
            )
        })
        .collect();
    builder.attach(root, nodes[0]);
    for pair in nodes.windows(2) {
        builder.attach(pair[0], pair[1]);
    }
    let skin = builder.add_skin(Skin {
        joints: nodes.clone(),
        inverse_bind: (0..BONES)
            .map(|index| Mat4::from_translation(Vec3::new(0.0, -(index as f32), 0.0)))
            .collect(),
        skeleton: Some(root),
        ..Skin::new("synthetic")
    });
    builder.model_mut().nodes[root.index()].skin = Some(skin);
    let mut document = document(&nodes);
    document.mesh = mesh;
    document.root = root;
    edit(&mut document);
    builder.extend_with(document);
    builder.finish().expect("the synthetic model is valid")
}

fn written_twice(model: &Model, exporter: impl Exporter + Copy + 'static) -> (Vec<u8>, Model) {
    let first = export(model, exporter).expect("the model is written");
    let again = native(first.clone()).expect("the written model is read");
    let second = export(&again, exporter).expect("the model read back is written");
    let differs = first.iter().zip(&second).position(|(one, two)| one != two);
    assert_eq!(
        (differs, second.len()),
        (None, first.len()),
        "the second write repeats the first"
    );
    (first, again)
}

#[derive(Clone, Copy)]
enum Rotations {
    Exact,
    Near,
}

fn assert_same_document(before: &Pmx, after: &Pmx, rotations: Rotations) {
    assert_eq!(after.name, before.name);
    assert_eq!(after.textures, before.textures);
    assert_eq!(after.vertices, before.vertices);
    assert_eq!(after.materials, before.materials);
    assert_eq!(after.morphs, before.morphs);
    assert_eq!(after.display_frames, before.display_frames);
    assert_eq!(after.soft_bodies, before.soft_bodies);
    let names = |pmx: &Pmx| {
        pmx.bones
            .iter()
            .map(|bone| bone.name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(after), names(before));
    let settled = |rotation: Quat, source: Quat| match rotations {
        Rotations::Exact => rotation,
        Rotations::Near => {
            assert!(rotation.abs_diff_eq(source, 1e-5), "{rotation} {source}");
            source
        }
    };
    let bodies: Vec<PmxRigidBody> = after
        .rigid_bodies
        .iter()
        .zip(&before.rigid_bodies)
        .map(|(body, source)| PmxRigidBody {
            rotation: settled(body.rotation, source.rotation),
            ..body.clone()
        })
        .collect();
    assert_eq!(bodies, before.rigid_bodies);
    let joints: Vec<PmxJoint> = after
        .joints
        .iter()
        .zip(&before.joints)
        .map(|(joint, source)| PmxJoint {
            rotation: settled(joint.rotation, source.rotation),
            ..joint.clone()
        })
        .collect();
    assert_eq!(joints, before.joints);
}

fn assert_forced_framing(automatic: &[u8], forced: &[u8]) {
    assert_eq!(version_of(automatic), 2.0);
    assert_eq!(version_of(forced), 2.1);
    assert_eq!(forced[..4], automatic[..4]);
    assert_eq!(forced[8..automatic.len()], automatic[8..]);
    assert_eq!(forced[automatic.len()..], 0_i32.to_le_bytes());
}

#[test]
fn version_1_features_write_pmx_2_0() {
    let model = synthetic(|_| {});
    assert_eq!(pmx(&model).required_version(), PmxVersion::V2_0);
    let (bytes, again) = written_twice(&model, PmxExporter);
    assert_eq!(version_of(&bytes), 2.0);
    assert_eq!(pmx(&again).vertices.additional_uvs.len(), 4);
    assert_same_document(pmx(&model), pmx(&again), Rotations::Exact);
}

#[test]
fn each_2_1_feature_round_trips_as_pmx_2_1() {
    let features: [(&str, Edit); 10] = [
        ("qdef", |pmx| {
            pmx.vertices.weights[3] = PmxWeight::Quaternion
        }),
        ("flip morph", |pmx| {
            pmx.morphs[SLOT].offsets = PmxOffsets::Flip(vec![
                PmxGroupOffset {
                    morph: 0,
                    ratio: 1.0,
                },
                PmxGroupOffset {
                    morph: 1,
                    ratio: 0.5,
                },
            ]);
        }),
        ("impulse morph", |pmx| {
            pmx.morphs[SLOT].offsets = PmxOffsets::Impulse(vec![PmxImpulseOffset {
                body: 1,
                local: true,
                velocity: Vec3::new(1.0, 2.0, 3.0),
                torque: Vec3::new(0.5, -0.25, 0.0),
            }]);
        }),
        ("6dof joint", |pmx| {
            pmx.joints[0].kind = PmxJointKind::SixDof.byte()
        }),
        ("p2p joint", |pmx| {
            pmx.joints[0].kind = PmxJointKind::PointToPoint.byte()
        }),
        ("cone twist joint", |pmx| {
            pmx.joints[0].kind = PmxJointKind::ConeTwist.byte();
        }),
        ("slider joint", |pmx| {
            pmx.joints[0].kind = PmxJointKind::Slider.byte()
        }),
        ("hinge joint", |pmx| {
            pmx.joints[0].kind = PmxJointKind::Hinge.byte()
        }),
        ("soft body", |pmx| pmx.soft_bodies.push(soft_body())),
        ("material drawing flags", |pmx| {
            let flags = &mut pmx.materials[0].flags;
            flags.set(PmxMaterialFlags::VERTEX_COLOR, true);
            flags.set(PmxMaterialFlags::POINTS, true);
            flags.set(PmxMaterialFlags::LINES, true);
        }),
    ];
    for (feature, edit) in features {
        let model = synthetic(edit);
        assert_eq!(
            pmx(&model).required_version(),
            PmxVersion::V2_1,
            "{feature}"
        );
        let (bytes, again) = written_twice(&model, PmxExporter);
        assert_eq!(version_of(&bytes), 2.1, "{feature}");
        assert_same_document(pmx(&model), pmx(&again), Rotations::Exact);
    }
}

#[test]
fn forced_2_1_reads_back_like_2_0() {
    let model = synthetic(|_| {});
    let automatic = export(&model, PmxExporter).expect("the 2.0 file");
    let (forced, again) = written_twice(&model, Pmx21Exporter);
    assert_forced_framing(&automatic, &forced);
    assert_same_document(pmx(&model), pmx(&again), Rotations::Exact);
}

#[test]
fn sample_round_trips_as_pmx_2_0() {
    let model = native(MIKU.to_vec()).expect("the sample model is read");
    let bytes = export(&model, PmxExporter).expect("the sample is written");
    assert_eq!(version_of(&bytes), 2.0);
    let again = native(bytes).expect("the written sample is read");
    assert_same_document(pmx(&model), pmx(&again), Rotations::Near);
}

#[test]
fn forced_sample_reads_back_identically() {
    let model = native(MIKU.to_vec()).expect("the sample model is read");
    let automatic = export(&model, PmxExporter).expect("the 2.0 file");
    let forced = export(&model, Pmx21Exporter).expect("the 2.1 file");
    assert_forced_framing(&automatic, &forced);
    let automatic = native(automatic).expect("the 2.0 file is read");
    let forced = native(forced).expect("the 2.1 file is read");
    assert_same_document(pmx(&automatic), pmx(&forced), Rotations::Exact);
}

#[test]
fn invalid_documents_are_refused_on_write() {
    let cases: [(&str, Edit); 8] = [
        ("anchor body", |pmx| {
            let mut soft = soft_body();
            soft.anchors[0].body = 2;
            pmx.soft_bodies.push(soft);
        }),
        ("anchor vertex", |pmx| {
            let mut soft = soft_body();
            soft.anchors[0].vertex = 4;
            pmx.soft_bodies.push(soft);
        }),
        ("pin", |pmx| {
            let mut soft = soft_body();
            soft.pins.push(9);
            pmx.soft_bodies.push(soft);
        }),
        ("soft body material", |pmx| {
            let mut soft = soft_body();
            soft.material = Some(1);
            pmx.soft_bodies.push(soft);
        }),
        ("impulse body", |pmx| {
            pmx.morphs[SLOT].offsets = PmxOffsets::Impulse(vec![PmxImpulseOffset {
                body: 2,
                local: false,
                velocity: Vec3::ZERO,
                torque: Vec3::ZERO,
            }]);
        }),
        ("flip morph", |pmx| {
            pmx.morphs[SLOT].offsets = PmxOffsets::Flip(vec![PmxGroupOffset {
                morph: 8,
                ratio: 1.0,
            }]);
        }),
        ("joint kind", |pmx| pmx.joints[0].kind = 6),
        ("additional uv sets", |pmx| {
            pmx.vertices.additional_uvs.push(vec![Vec4::ZERO; VERTICES]);
        }),
    ];
    for (case, edit) in cases {
        let model = synthetic(edit);
        assert!(
            matches!(export(&model, PmxExporter), Err(Error::Invalid(_))),
            "{case}"
        );
    }
}

fn soft_body_file() -> Vec<u8> {
    export(
        &synthetic(|pmx| pmx.soft_bodies.push(soft_body())),
        PmxExporter,
    )
    .expect("the soft body file")
}

#[test]
fn hostile_soft_bodies_are_refused() {
    let bytes = soft_body_file();
    let end = bytes.len();
    let anchor_body = end - 8;
    let anchor_vertex = end - 7;
    let pin = end - 1;
    let corrupt = |at: usize, value: u8| {
        let mut corrupted = bytes.clone();
        corrupted[at] = value;
        corrupted
    };
    let cases = [
        ("anchor body past the bodies", corrupt(anchor_body, 2)),
        ("anchor without a body", corrupt(anchor_body, 0xff)),
        ("anchor vertex", corrupt(anchor_vertex, 4)),
        ("pin vertex", corrupt(pin, 4)),
        ("truncated anchor", bytes[..anchor_vertex].to_vec()),
        ("truncated pins", bytes[..pin].to_vec()),
    ];
    assert!(native(bytes.clone()).is_ok());
    for (case, hostile) in cases {
        assert!(matches!(native(hostile), Err(Error::Damaged(_))), "{case}");
    }
}
