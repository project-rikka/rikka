use std::sync::{Arc, LazyLock};

use rikka_mmd::{
    FRAMES_PER_SECOND, MMD_UNIT_SCALE, Pmx, PmxExporter, PmxImporter, PmxOffsets, Puppet, Vmd,
    VmdCameraFrame, VmdCameraInterpolation, VmdExporter, VmdImporter, VmdPropertyFrame,
};
use rikka_model_core::glam::Vec3;
use rikka_model_core::{
    Animation, Channel, CoordinateSystem, Curve, Export, ExportOptions, Import, Instance,
    MatcapBlend, Material, MemoryResolver, MemorySink, Mesh, Model, ModelBuilder, Node, Registry,
    Shading, Sink, Submesh, Track, Transform, Vertices,
};

const MIKU: &[u8] = include_bytes!("../assets/mmd/miku.pmx");
const MOTION: &[u8] = include_bytes!("../assets/mmd/motion.vmd");

const TEXTURES: [(&str, &[u8]); 10] = [
    ("tex/skin.png", include_bytes!("../assets/mmd/tex/skin.png")),
    ("tex/eye.png", include_bytes!("../assets/mmd/tex/eye.png")),
    ("tex/hair.png", include_bytes!("../assets/mmd/tex/hair.png")),
    (
        "tex/other.png",
        include_bytes!("../assets/mmd/tex/other.png"),
    ),
    ("tex/B.png", include_bytes!("../assets/mmd/tex/B.png")),
    ("spa/hair.png", include_bytes!("../assets/mmd/spa/hair.png")),
    (
        "toon/skin.png",
        include_bytes!("../assets/mmd/toon/skin.png"),
    ),
    (
        "toon/hair.png",
        include_bytes!("../assets/mmd/toon/hair.png"),
    ),
    ("toon/B.png", include_bytes!("../assets/mmd/toon/B.png")),
    ("toon/W.png", include_bytes!("../assets/mmd/toon/W.png")),
];

static BESIDE: LazyLock<Arc<MemoryResolver>> = LazyLock::new(|| {
    let mut resolver = MemoryResolver::new();
    for (path, bytes) in TEXTURES {
        resolver.insert(path, bytes.to_vec());
    }
    Arc::new(resolver)
});

fn registry() -> Arc<Registry> {
    Arc::new(
        Registry::new()
            .with_importer(PmxImporter)
            .with_importer(VmdImporter)
            .with_exporter(PmxExporter)
            .with_exporter(VmdExporter),
    )
}

fn beside() -> Arc<MemoryResolver> {
    Arc::clone(&BESIDE)
}

fn miku() -> Model {
    Import::named_bytes("miku.pmx", MIKU.to_vec(), registry())
        .resolver(beside())
        .run()
        .expect("the sample model is read")
}

fn motion() -> Model {
    Import::named_bytes("motion.vmd", MOTION.to_vec(), registry())
        .run()
        .expect("the sample motion is read")
}

fn written(model: &Model, name: &str) -> Model {
    let sink = Arc::new(MemorySink::new());
    let exported = Export::new(model, registry())
        .options(ExportOptions::new())
        .sink(Arc::clone(&sink) as Arc<dyn Sink>)
        .to_bytes(name)
        .expect("the model is written");
    assert!(exported.warnings.is_empty(), "{:?}", exported.warnings);
    Import::named_bytes(name, exported.bytes, registry())
        .resolver(beside())
        .run()
        .expect("the written model is read again")
}

#[test]
fn document_reads_as_model() {
    let model = miku();
    assert_eq!(model.metadata.importer, "PMX");
    assert_eq!(model.meshes.len(), 1, "a document draws one mesh");
    assert!(model.materials.len() > 5, "{}", model.materials.len());
    assert!(
        model.triangle_count() > 10_000,
        "{}",
        model.triangle_count()
    );
    assert_eq!(
        model.meshes[0].submeshes.len(),
        model.materials.len(),
        "every material draws one run of the mesh"
    );
    model.validate().expect("the model holds together");
}

#[test]
fn document_converts_to_canonical_metres() {
    let model = miku();
    assert_eq!(model.metadata.coordinates, CoordinateSystem::CANONICAL);
    assert_eq!(model.metadata.unit_scale, 1.0);

    let bounds = model.bounds();
    let height = bounds.max.y - bounds.min.y;
    assert!((1.0..2.5).contains(&height), "{height} metres tall");
}

#[test]
fn reads_every_external_texture() {
    let model = miku();
    assert!(
        model.warnings.is_empty(),
        "nothing is missing: {:?}",
        model.warnings
    );
    assert!(!model.images.is_empty());
    assert!(
        model
            .materials
            .iter()
            .any(|material| material.textures.base_color.is_some())
    );
    assert!(
        model
            .materials
            .iter()
            .all(|material| matches!(material.shading, Shading::Toon(_))),
        "every material of a document is toon shaded"
    );
}

#[test]
fn bones_become_skin() {
    let model = miku();
    let pmx = model.extensions.get::<Pmx>().expect("the document");
    assert_eq!(model.skins.len(), 1);
    assert_eq!(model.skins[0].joint_count(), pmx.bones.len());
    assert!(pmx.bones.len() > 50, "{}", pmx.bones.len());

    for bone in &pmx.bones {
        assert_eq!(model.node(bone.node).name, bone.name);
    }
    let mesh = model
        .meshes
        .iter()
        .find(|mesh| mesh.vertices.is_skinned())
        .expect("a skinned mesh");
    assert!(
        mesh.vertices
            .weights
            .iter()
            .all(|weight| (weight.element_sum() - 1.0).abs() < 1e-3)
    );
}

#[test]
fn morphs_align_with_mesh_targets() {
    let model = miku();
    let pmx = model.extensions.get::<Pmx>().expect("the document");
    let targets = &model.meshes[0].morph_targets;
    assert_eq!(pmx.morphs.len(), targets.len());
    for (morph, target) in pmx.morphs.iter().zip(targets) {
        assert_eq!(morph.name, target.name);
    }
    assert!(
        pmx.morphs
            .iter()
            .zip(targets)
            .any(
                |(morph, target)| matches!(morph.offsets, PmxOffsets::Vertex)
                    && !target.vertices.is_empty()
            ),
        "a document moves vertices with at least one of its morphs"
    );
}

#[test]
fn format_extension_round_trips() {
    let model = miku();
    let pmx = model.extensions.get::<Pmx>().expect("the document");
    assert!(!pmx.display_frames.is_empty(), "the panels MMD shows");
    assert!(!pmx.rigid_bodies.is_empty(), "the bodies of its physics");
    assert!(!pmx.joints.is_empty(), "the joints between them");
    assert!(
        pmx.bones.iter().any(|bone| bone.ik.is_some()),
        "a model has bones that reach for a target"
    );
    assert!(
        pmx.bones.iter().any(|bone| bone.append.is_some()),
        "a model has bones that follow another one"
    );
}

#[test]
fn missing_texture_keeps_other_textures() {
    let mut resolver = MemoryResolver::new();
    for (path, bytes) in TEXTURES.iter().skip(1) {
        resolver.insert(path, bytes.to_vec());
    }
    let model = Import::named_bytes("miku.pmx", MIKU.to_vec(), registry())
        .resolver(Arc::new(resolver))
        .run()
        .expect("the sample model is read without one of its textures");
    assert!(
        !model.warnings.is_empty(),
        "the missing texture is warned of"
    );

    let sink = Arc::new(MemorySink::new());
    let exported = Export::new(&model, registry())
        .sink(Arc::clone(&sink) as Arc<dyn Sink>)
        .to_bytes("miku.pmx")
        .expect("the model is written");
    let again = Import::named_bytes("miku.pmx", exported.bytes, registry())
        .resolver(beside())
        .run()
        .expect("the written model is read again");

    let one = model.extensions.get::<Pmx>().expect("the document");
    let two = again.extensions.get::<Pmx>().expect("the document again");
    assert_eq!(two.textures, one.textures);
    for (before, after) in one.materials.iter().zip(&two.materials) {
        assert_eq!(
            after.texture, before.texture,
            "material \"{}\" is drawn with the texture it named",
            before.name
        );
        assert_eq!(after.sphere, before.sphere);
        assert_eq!(after.toon, before.toon);
    }
}
#[test]
fn document_round_trips() {
    let model = miku();
    let again = written(&model, "miku.pmx");
    assert_eq!(again.vertex_count(), model.vertex_count());
    assert_eq!(again.triangle_count(), model.triangle_count());
    assert_eq!(again.materials.len(), model.materials.len());

    let one = model.extensions.get::<Pmx>().expect("the document");
    let two = again.extensions.get::<Pmx>().expect("the document again");
    assert_eq!(two.name, one.name);
    assert_eq!(two.bones.len(), one.bones.len());
    assert_eq!(two.morphs.len(), one.morphs.len());
    assert_eq!(two.rigid_bodies.len(), one.rigid_bodies.len());
    assert_eq!(two.joints.len(), one.joints.len());
    assert_eq!(two.display_frames.len(), one.display_frames.len());
    for (before, after) in one.bones.iter().zip(&two.bones) {
        assert_eq!(after.name, before.name);
        assert!(
            after.position.abs_diff_eq(before.position, 1e-4),
            "{} moved from {} to {}",
            before.name,
            before.position,
            after.position
        );
    }
}

#[test]
fn sphere_blend_round_trips() {
    let mut model = miku();
    let sphered = model
        .materials
        .iter()
        .position(|material| material.textures.matcap.is_some())
        .expect("the hair carries a sphere map");
    let blend = |model: &Model| match model.materials[sphered].shading {
        Shading::Toon(toon) => toon.matcap_blend,
        Shading::Pbr(_) => panic!("PMX materials are toon"),
    };
    assert_eq!(blend(&model), MatcapBlend::Add);
    if let Shading::Toon(toon) = &mut model.materials[sphered].shading {
        toon.matcap_blend = MatcapBlend::Multiply;
    }
    model.extensions.remove::<Pmx>();
    let again = written(&model, "miku.pmx");
    assert_eq!(blend(&again), MatcapBlend::Multiply);
    assert!(again.materials[sphered].textures.matcap.is_some());
}

#[test]
fn motion_reads_keyframes() {
    let model = motion();
    assert_eq!(model.metadata.importer, "VMD");
    let motion = model.extensions.get::<Vmd>().expect("the motion");
    assert!(!motion.bones.is_empty(), "a motion moves bones");
    assert!(!motion.morphs.is_empty(), "a motion makes morphs");
    assert!(motion.duration() > 1.0, "{}", motion.duration());
    assert!(!model.animations.is_empty(), "a motion is an animation");
}

#[test]
fn bone_tracks_match_motion() {
    let model = motion();
    let motion = model.extensions.get::<Vmd>().expect("the motion");
    let tracks = motion.bone_tracks();
    let last = motion.last_frame();
    for bone_frame in motion.bones.iter().step_by(37) {
        for frame in [
            0,
            bone_frame.frame / 2,
            bone_frame.frame,
            bone_frame.frame + 1,
            last + 10,
        ] {
            assert_eq!(
                tracks.at(&bone_frame.bone, frame as f32 + 0.25),
                motion.bone_at(&bone_frame.bone, frame as f32 + 0.25),
                "{} at {frame}",
                bone_frame.bone
            );
            assert_eq!(
                tracks.key(&bone_frame.bone, frame),
                motion
                    .bones
                    .iter()
                    .find(|key| key.bone == bone_frame.bone && key.frame == frame),
                "{} on {frame}",
                bone_frame.bone
            );
        }
    }
    assert!(tracks.at("no such bone", 1.0).is_none());
}

#[test]
fn motion_round_trips() {
    let model = motion();
    let again = written(&model, "motion.vmd");
    let one = model.extensions.get::<Vmd>().expect("the motion");
    let two = again.extensions.get::<Vmd>().expect("the motion again");
    assert_eq!(two.model, one.model);
    assert_eq!(two.bones.len(), one.bones.len());
    assert_eq!(two.morphs.len(), one.morphs.len());
    assert_eq!(two.properties.len(), one.properties.len());
    for (before, after) in one.bones.iter().zip(&two.bones) {
        assert_eq!(after.bone, before.bone);
        assert_eq!(after.frame, before.frame);
        assert_eq!(after.interpolation, before.interpolation);
        assert!(after.translation.abs_diff_eq(before.translation, 1e-4));
        assert!(
            after.rotation.dot(before.rotation).abs() > 1.0 - 1e-4,
            "{} on frame {} turns to {} rather than {}",
            before.bone,
            before.frame,
            after.rotation,
            before.rotation
        );
    }
}

#[test]
fn motion_drives_model() {
    let model = miku();
    let motion = motion();
    let vmd = motion.extensions.get::<Vmd>().expect("the motion");
    let animation = vmd.animation(&model);
    assert!(
        animation.channels.len() > 20,
        "{}",
        animation.channels.len()
    );
    assert!(animation.duration() > 1.0, "{}", animation.duration());

    let mut posed = model;
    let name = animation.name.clone();
    posed.animations.push(animation);
    let posed = Arc::new(posed);
    let id = posed.find_animation(&name).expect("the motion");
    let mut instance = Instance::new(posed);
    instance.play(id);
    instance.advance(2.0);
    instance.advance(2.0);

    let model = Arc::clone(instance.model());
    let centre = model.find_node("センター").expect("the centre bone");
    assert!(
        instance.world(centre).w_axis.truncate() != Vec3::ZERO,
        "the motion moves the model"
    );
}

#[test]
fn baked_motion_is_exact_at_keys_and_follows_curves_between() {
    let model = miku();
    let motion = motion();
    let vmd = motion.extensions.get::<Vmd>().expect("the motion");
    let tracks = vmd.bone_tracks();
    let animation = vmd.animation(&model);
    let mut keys_checked = 0;
    for channel in &animation.channels {
        let node = model.node(channel.target);
        let track = tracks.track(&node.name);
        let (Some(first), Some(last)) = (track.first(), track.last()) else {
            continue;
        };
        let rest = node.transform.translation;
        for frame in first.frame..=last.frame {
            let time = frame as f32 / FRAMES_PER_SECOND;
            let keyed = tracks.key(&node.name, frame).is_some();
            let (translation, rotation) =
                tracks.at(&node.name, frame as f32).expect("a keyed bone");
            match &channel.track {
                Track::Translation(curve) => {
                    let baked = curve.sample(time).expect("a baked translation");
                    let expected = rest + translation;
                    match keyed {
                        true => assert_eq!(baked, expected, "{} on {frame}", node.name),
                        false => assert!(
                            baked.abs_diff_eq(expected, 1e-3 * (1.0 + expected.length())),
                            "{} at {frame}: {baked} rather than {expected}",
                            node.name
                        ),
                    }
                }
                Track::Rotation(curve) => {
                    let baked = curve.sample(time).expect("a baked rotation");
                    match keyed {
                        true => assert!(
                            baked.dot(rotation).abs() > 1.0 - 1e-6,
                            "{} on {frame}: {baked} rather than {rotation}",
                            node.name
                        ),
                        false => assert!(
                            2.0 * (baked.inverse() * rotation).xyz().length() < 1e-3,
                            "{} at {frame}: {baked} rather than {rotation}",
                            node.name
                        ),
                    }
                }
                _ => continue,
            }
            keys_checked += usize::from(keyed);
        }
    }
    assert!(keys_checked > 100, "{keys_checked}");
}

#[test]
fn ik_bone_reaches_target() {
    let model = Arc::new(miku());
    let pmx = model.extensions.get::<Pmx>().expect("the document");
    let ik_bone = pmx
        .bone("左足ＩＫ")
        .expect("the bone the left leg reaches with");
    let ik = ik_bone.ik.as_ref().expect("what it reaches with");
    let target = pmx.bones[ik.target].node;

    let mut instance = Instance::new(Arc::clone(&model));
    let rest = instance
        .world(ik_bone.node)
        .w_axis
        .truncate()
        .distance(instance.world(target).w_axis.truncate());
    assert!(rest < 0.02, "a leg rests where it reaches: {rest}");

    let mut pose = instance.pose().clone();
    let local = pose.local(ik_bone.node);
    pose.set_local(
        ik_bone.node,
        Transform {
            translation: local.translation + Vec3::new(0.0, 0.08, 0.10),
            ..local
        },
    );
    *instance.pose_mut() = pose;
    instance.refresh();

    let reached = instance.world(ik_bone.node).w_axis.truncate();
    let landed = instance.world(target).w_axis.truncate();
    let knee = model.find_node("左ひざ").expect("the knee that bends");
    assert!(
        reached.distance(landed) < 0.01,
        "the leg bends until the ankle stands where the bone that reaches for it does: \
         {reached} against {landed}"
    );
    assert!(
        instance.world(knee).w_axis.truncate().z > 0.0,
        "the knee bends forward rather than backwards"
    );
}

#[test]
fn knee_folds_past_right_angle() {
    let model = Arc::new(miku());
    let pmx = model.extensions.get::<Pmx>().expect("the document");
    let ik_bone = pmx
        .bone("左足ＩＫ")
        .expect("the bone the left leg reaches with");
    let target = pmx.bones[ik_bone.ik.as_ref().expect("what it reaches with").target].node;
    let place = |instance: &Instance, name: &str| {
        let node = model.find_node(name).expect("a leg bone");
        instance.world(node).w_axis.truncate()
    };

    let mut instance = Instance::new(Arc::clone(&model));
    let mut pose = instance.pose().clone();
    let local = pose.local(ik_bone.node);
    pose.set_local(
        ik_bone.node,
        Transform {
            translation: local.translation + Vec3::new(0.0, 0.55, 0.2),
            ..local
        },
    );
    *instance.pose_mut() = pose;
    instance.refresh();

    let reached = instance.world(ik_bone.node).w_axis.truncate();
    let landed = instance.world(target).w_axis.truncate();
    assert!(
        reached.distance(landed) < 0.01,
        "{reached} against {landed}"
    );
    let (hip, knee, ankle) = (
        place(&instance, "左足"),
        place(&instance, "左ひざ"),
        place(&instance, "左足首"),
    );
    let folded = (knee - hip).angle_between(ankle - knee).to_degrees();
    assert!(folded > 100.0, "the knee folds only {folded} degrees");
}

#[test]
fn standalone_camera_motion_faces_target() {
    let motion = Vmd {
        cameras: vec![VmdCameraFrame {
            frame: 0,
            distance: -45.0,
            target: Vec3::new(0.0, 10.0, 0.0),
            rotation: Vec3::ZERO,
            interpolation: VmdCameraInterpolation::default(),
            angle: 30,
            perspective: true,
        }],
        ..Vmd::default()
    };
    let mut builder = ModelBuilder::new()
        .coordinates(CoordinateSystem::LEFT_HANDED_Y_UP)
        .unit_scale(MMD_UNIT_SCALE);
    builder.extend_with(motion);
    let read = Arc::new(written(
        &builder.finish().expect("a camera motion"),
        "camera.vmd",
    ));

    let camera = read.find_node("camera").expect("the camera it films with");
    let mut instance = Instance::new(Arc::clone(&read));
    instance.play(read.animation_id(0).expect("the motion"));
    instance.seek(0.0);
    let camera_world = instance.world(camera);
    let eye = camera_world.w_axis.truncate();
    let target = Vec3::new(0.0, 10.0 * MMD_UNIT_SCALE, 0.0);
    assert!((eye - Vec3::new(0.0, 0.8, 3.6)).length() < 1e-3, "{eye}");
    let facing = camera_world.transform_vector3(Vec3::NEG_Z).normalize();
    assert!(facing.dot((target - eye).normalize()) > 0.999, "{facing}");
}

#[test]
fn disabled_ik_keeps_keyframed_leg() {
    let model = Arc::new(miku());
    let pmx = model.extensions.get::<Pmx>().expect("the document");
    let (bone, ik_bone) = pmx
        .bones
        .iter()
        .enumerate()
        .find(|(_, bone)| bone.name == "左足ＩＫ")
        .expect("the bone the left leg reaches with");
    let target = pmx.bones[ik_bone.ik.as_ref().expect("what it reaches with").target].node;
    let motion = Vmd {
        properties: vec![
            VmdPropertyFrame {
                frame: 0,
                visible: true,
                ik: vec![("左足ＩＫ".into(), true)],
            },
            VmdPropertyFrame {
                frame: 30,
                visible: false,
                ik: vec![("左足ＩＫ".into(), false)],
            },
        ],
        ..Vmd::default()
    };
    let switches = motion.switches(&model);
    assert!(switches.visible(0.5));
    assert!(!switches.visible(1.0), "the model hides from frame 30");

    let mut instance = Instance::new(Arc::clone(&model));
    let mut pose = instance.pose().clone();
    let local = pose.local(ik_bone.node);
    pose.set_local(
        ik_bone.node,
        Transform {
            translation: local.translation + Vec3::new(0.0, 0.08, 0.10),
            ..local
        },
    );
    *instance.pose_mut() = pose;

    let mut reached_at = |time: f32| {
        let puppet = instance
            .rig_as_mut::<Puppet>()
            .expect("a document is posed by its puppet");
        switches.apply(time, puppet);
        assert_eq!(puppet.is_reaching(bone), time < 1.0);
        instance.refresh();
        instance
            .world(ik_bone.node)
            .w_axis
            .truncate()
            .distance(instance.world(target).w_axis.truncate())
    };
    assert!(reached_at(0.0) < 0.01, "the leg reaches while switched on");
    assert!(
        reached_at(1.0) > 0.05,
        "and stays where its keyframes put it once switched off"
    );
}

fn triangle() -> Model {
    let mut builder = ModelBuilder::new();
    let material = builder.add_material(Material::new("skin"));
    let mesh = builder.add_mesh(Mesh {
        vertices: Vertices {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            normals: vec![Vec3::Z; 3],
            uvs: vec![rikka_model_core::glam::Vec2::ZERO; 3],
            ..Vertices::default()
        },
        indices: vec![0, 1, 2],
        submeshes: vec![Submesh {
            name: "skin".into(),
            first_index: 0,
            index_count: 3,
            material: Some(material),
            ..Submesh::default()
        }],
        ..Mesh::new("triangle")
    });
    builder.add_node(Node::new("センター").mesh(mesh));
    builder.finish().expect("a triangle is a model")
}

#[test]
fn built_model_exports() {
    let model = triangle();
    let again = written(&model, "triangle.pmx");
    assert_eq!(again.metadata.importer, "PMX");
    assert_eq!(again.vertex_count(), model.vertex_count());
    assert_eq!(again.triangle_count(), model.triangle_count());
    assert_eq!(again.materials.len(), 1);

    let pmx = again.extensions.get::<Pmx>().expect("the document");
    assert_eq!(
        pmx.bones.len(),
        1,
        "a document hangs from at least one bone"
    );
    assert_eq!(pmx.bones[0].name, "センター");
    let bounds = again.bounds();
    assert!(bounds.max.x - bounds.min.x > 0.9, "{bounds:?}");
}

#[test]
fn animation_exports_as_vmd() {
    let mut model = triangle();
    let node = model.find_node("センター").expect("the centre bone");
    model.animations.push(Animation {
        name: "wave".into(),
        channels: vec![Channel {
            target: node,
            track: Track::Translation(Curve::linear(
                vec![0.0, 1.0],
                vec![Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0)],
            )),
        }],
    });

    let again = written(&model, "wave.vmd");
    let motion = again.extensions.get::<Vmd>().expect("the motion");
    assert_eq!(motion.bones.len(), 2);
    assert_eq!(motion.bones[0].bone, "センター");
    assert_eq!(motion.bones[0].frame, 0);
    assert_eq!(motion.bones[1].frame, 30);
    assert!(
        motion.bones[1].translation.y > 0.9,
        "the metre the animation moves through comes back a metre: {}",
        motion.bones[1].translation.y
    );
}
