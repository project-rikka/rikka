use std::sync::Arc;

use rikka_model_core::glam::{Quat, Vec3};
use rikka_model_core::{
    Export, ExportOptions, HumanBone, Import, Instance, MemorySink, Model, OutlineWidth, Shading,
    Sink,
};
use rikka_vrm::{Constraint, ExpressionPreset, Expressive, LookAtKind, VrmVersion};

use crate::support::{registry, vrm};

const AVATAR: &[u8] = include_bytes!("../assets/avatar.vrm");

fn avatar() -> Model {
    Import::named_bytes("avatar.vrm", AVATAR.to_vec(), registry())
        .run()
        .expect("the sample avatar is read")
}

fn written(model: &Model) -> Model {
    let sink = Arc::new(MemorySink::new());
    let exported = Export::new(model, registry())
        .options(ExportOptions::new().embed_resources(true))
        .sink(Arc::clone(&sink) as Arc<dyn Sink>)
        .to_bytes("avatar.vrm")
        .expect("the avatar is written");
    Import::named_bytes("avatar.vrm", exported.bytes, registry())
        .run()
        .expect("the written avatar is read again")
}

#[test]
fn sample_avatar_reads() {
    let model = avatar();
    assert_eq!(model.metadata.importer, "VRM");

    let vrm = vrm(&model);
    assert_eq!(vrm.version, VrmVersion::V1);
    assert_eq!(vrm.meta.name, "VRM1_Constraint_Twist_Sample");
    assert_eq!(vrm.meta.authors, vec!["pixiv Inc.".to_owned()]);
    assert!(vrm.meta.allow_redistribution);
    assert!(vrm.meta.thumbnail.is_some());

    assert_eq!(vrm.humanoid.missing().count(), 0);
    let head = vrm.bone(HumanBone::Head).expect("a head");
    assert!(model.node(head).name.contains("Head"));

    assert!(vrm.expression(&ExpressionPreset::Happy.into()).is_some());
    assert!(!vrm.spring_bone.springs.is_empty());
    assert!(!vrm.constraints.is_empty());
    assert_eq!(vrm.look_at.kind, LookAtKind::Bone);
}

#[test]
fn avatar_materials_are_toon() {
    let model = avatar();
    let toon = model
        .materials
        .iter()
        .filter_map(|material| match material.shading {
            Shading::Toon(toon) => Some((material, toon)),
            Shading::Pbr(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(toon.len(), model.materials.len());

    let (material, toon_material) = toon
        .iter()
        .find(|(material, _)| material.name == "Body_00_SKIN")
        .expect("the skin");
    assert!(material.textures.shade.is_some());
    assert!(matches!(toon_material.outline.width, OutlineWidth::World(width) if width > 0.0));
    assert!(toon_material.shading_toony > 0.0);
}

#[test]
fn vrm_metadata_round_trips() {
    let model = avatar();
    let again = written(&model);

    let (before, after) = (vrm(&model), vrm(&again));
    assert_eq!(before.meta, after.meta);
    assert_eq!(before.humanoid, after.humanoid);
    assert_eq!(before.look_at, after.look_at);
    assert_eq!(before.spring_bone, after.spring_bone);
    assert_eq!(before.constraints, after.constraints);
    assert_eq!(before.expressions.len(), after.expressions.len());
    assert_eq!(
        before.expression(&ExpressionPreset::Aa.into()),
        after.expression(&ExpressionPreset::Aa.into())
    );
    assert_eq!(model.materials, again.materials);
    assert_eq!(model.vertex_count(), again.vertex_count());
}

#[test]
fn expression_drives_bound_morphs() {
    let model = Arc::new(avatar());
    let bind = vrm(&model)
        .expression(&ExpressionPreset::Aa.into())
        .expect("the aa expression")
        .morph_binds
        .first()
        .copied()
        .expect("a morph target");

    let mut instance = Instance::new(Arc::clone(&model));
    assert_eq!(instance.morph_weights(bind.node)[bind.index as usize], 0.0);

    instance.set_expression(ExpressionPreset::Aa, 1.0);
    instance.refresh();
    assert_eq!(
        instance.morph_weights(bind.node)[bind.index as usize],
        bind.weight
    );

    instance.clear_expressions();
    instance.refresh();
    assert_eq!(instance.morph_weights(bind.node)[bind.index as usize], 0.0);
}

#[test]
fn look_at_turns_eyes_to_target() {
    let model = Arc::new(avatar());
    let eye = vrm(&model).bone(HumanBone::LeftEye).expect("a left eye");

    let mut instance = Instance::new(Arc::clone(&model));
    let ahead = instance.world(eye);

    instance.set_look_at_target(Some(Vec3::new(-2.0, 1.4, 2.0)));
    instance.refresh();
    let aside = instance.world(eye);
    assert_ne!(ahead, aside);

    instance.set_look_at_target(None);
    instance.refresh();
    assert_eq!(instance.world(eye), ahead);
}

#[test]
fn roll_constraint_follows_source() {
    let model = Arc::new(avatar());
    let roll = vrm(&model)
        .constraints
        .iter()
        .find(|node_constraint| matches!(node_constraint.constraint, Constraint::Roll { .. }))
        .copied()
        .expect("a roll constraint");
    let source = roll.constraint.source();

    let mut instance = Instance::new(Arc::clone(&model));
    let rest = instance.world(roll.node);

    let mut pose = instance.pose().clone();
    let mut local = pose.local(source);
    local.rotation *= Quat::from_rotation_x(0.6);
    pose.set_local(source, local);
    *instance.pose_mut() = pose;
    instance.refresh();

    assert_ne!(instance.world(roll.node), rest);
}
