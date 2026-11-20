use std::sync::Arc;

use animate_body::{Body, HumanBone, HumanPose, Mapping, muscles, muscles_of};
use rikka_humanoid::Anatomy;
use rikka_mmd::Puppet;
use rikka_model_core::glam::{Quat, Vec3};
use rikka_model_core::{Instance, Model, ModelBuilder, Node, Pose};

use crate::support::miku;

fn named(model: &Model, bone: HumanBone, mapping: &Mapping) -> String {
    model
        .node(mapping.node(bone).expect("a mapped bone"))
        .name
        .clone()
}

fn released(model: &Model, body: &Body) -> Instance {
    let mut instance = Instance::new(Arc::new(model.clone()));
    let puppet = instance.rig_as_mut::<Puppet>().expect("a PMX puppet");
    body.release(puppet);
    instance.refresh();
    instance
}

fn same_rotation(one: Quat, other: Quat) -> f32 {
    let one = one.normalize();
    let other = other.normalize();
    let other = match one.dot(other) < 0.0 {
        true => -other,
        false => other,
    };
    (one - other).length()
}

fn noise(seed: &mut u64) -> f32 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    ((*seed >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
}

#[test]
fn maps_required_bones_and_fingers() {
    let model = miku();
    let mapping =
        Mapping::from_anatomy(&model, &Anatomy::recognize(&model)).expect("miku is human");
    for bone in [
        HumanBone::Hips,
        HumanBone::Spine,
        HumanBone::Chest,
        HumanBone::Neck,
        HumanBone::Head,
        HumanBone::LeftEye,
        HumanBone::RightEye,
        HumanBone::LeftShoulder,
        HumanBone::RightShoulder,
        HumanBone::LeftToes,
        HumanBone::RightToes,
    ] {
        assert!(mapping.node(bone).is_some(), "{bone} is mapped");
    }
    for bone in HumanBone::ALL
        .iter()
        .skip_while(|bone| **bone != HumanBone::LeftUpperLeg)
    {
        assert!(mapping.node(*bone).is_some(), "{bone} is mapped");
    }
    assert!(mapping.node(HumanBone::Jaw).is_none());
    assert!(mapping.node(HumanBone::UpperChest).is_none());

    assert_eq!(named(&model, HumanBone::Hips, &mapping), "腰");
    assert_eq!(model.node(mapping.carrier()).name, "全ての親");
    assert_eq!(
        named(&model, HumanBone::LeftThumbMetacarpal, &mapping),
        "左親指０"
    );
    assert_eq!(
        named(&model, HumanBone::RightIndexDistal, &mapping),
        "右人指３"
    );
    assert_eq!(named(&model, HumanBone::LeftToes, &mapping), "左足先EX");
    let twist = |bone| {
        model
            .node(mapping.twist(bone).expect("a twist bone"))
            .name
            .clone()
    };
    assert_eq!(twist(HumanBone::LeftUpperArm), "左腕捩");
    assert_eq!(twist(HumanBone::RightLowerArm), "右手捩");
}

#[test]
fn rest_pose_round_trips_locals() {
    let model = miku();
    let body = Body::from_anatomy(&model, &Anatomy::recognize(&model)).expect("miku is human");
    let instance = released(&model, &body);
    let read = body.read(&instance);
    for (index, (one, other)) in read
        .muscles
        .iter()
        .zip(body.rest_pose().muscles)
        .enumerate()
    {
        assert!(
            (one - other).abs() < 1e-4,
            "muscle {index} reads {one} at rest, not {other}"
        );
    }

    let mut pose = Pose::rest(&model);
    for node in body.driven_nodes() {
        pose.set_local(*node, Default::default());
    }
    body.write(&read, &mut pose);
    for node in body.driven_nodes() {
        let rest = model.node(*node).transform;
        let local = pose.local(*node);
        let name = &model.node(*node).name;
        assert!(
            local.translation.distance(rest.translation) < 1e-4,
            "{name} is written at {:?}, not {:?}",
            local.translation,
            rest.translation
        );
        assert!(
            same_rotation(local.rotation, rest.rotation) < 1e-4,
            "{name} is written turned {:?}",
            local.rotation
        );
    }
}

#[test]
fn muscle_rotation_round_trip() {
    let model = miku();
    let body = Body::from_anatomy(&model, &Anatomy::recognize(&model)).expect("miku is human");
    let mut seed = 7;
    for bone in [
        HumanBone::Spine,
        HumanBone::Neck,
        HumanBone::Head,
        HumanBone::LeftShoulder,
        HumanBone::LeftUpperArm,
        HumanBone::RightUpperArm,
        HumanBone::LeftLowerArm,
        HumanBone::RightHand,
        HumanBone::LeftThumbMetacarpal,
        HumanBone::RightIndexProximal,
        HumanBone::LeftUpperLeg,
        HumanBone::RightLowerLeg,
        HumanBone::LeftFoot,
        HumanBone::RightToes,
    ] {
        for _ in 0..20 {
            let mut pose = HumanPose::default();
            for index in muscles_of(bone) {
                pose.muscles[index] = noise(&mut seed);
            }
            body.clamp(&mut pose, true);
            let rotation = body.local_rotation(bone, &pose);
            let mut back = HumanPose::default();
            body.set_local_rotation(bone, rotation, &mut back);
            for index in muscles_of(bone) {
                assert!(
                    (pose.muscles[index] - back.muscles[index]).abs() < 1e-3,
                    "{bone} muscle {index}: {} came back as {}",
                    pose.muscles[index],
                    back.muscles[index]
                );
            }
        }
    }
}

#[test]
fn written_pose_matches_skeleton() {
    let model = miku();
    let body = Body::from_anatomy(&model, &Anatomy::recognize(&model)).expect("miku is human");
    let mut pose = *body.rest_pose();
    pose.root_pos += Vec3::new(0.1, -0.05, 0.2);
    pose.root_rot = Quat::from_rotation_y(0.6) * Quat::from_rotation_x(0.2);
    let mut seed = 11;
    for (index, muscle) in muscles().iter().enumerate() {
        if body.has(muscle.bone) {
            pose.muscles[index] = (pose.muscles[index] + noise(&mut seed) * 0.5).clamp(-1.0, 1.0);
        }
    }

    let mut instance = released(&model, &body);
    body.write(&pose, instance.pose_mut());
    instance.refresh();
    let worlds = body.fk(&pose);
    for (bone, joint) in worlds.iter() {
        let node = body.mapping().node(bone).expect("a mapped bone");
        let placed = instance.world(node).w_axis.truncate();
        assert!(
            placed.distance(joint.position) < 1e-4,
            "{bone} stands at {placed:?}, the body says {:?}",
            joint.position
        );
    }

    let read = body.read(&instance);
    assert!(read.root_pos.distance(pose.root_pos) < 1e-4);
    assert!(same_rotation(read.root_rot, pose.root_rot) < 1e-4);
    for (index, muscle) in muscles().iter().enumerate() {
        if body.has(muscle.bone) {
            assert!(
                (read.muscles[index] - pose.muscles[index]).abs() < 1e-3,
                "{} {:?} reads {} after writing {}",
                muscle.bone,
                muscle.dof,
                read.muscles[index],
                pose.muscles[index]
            );
        }
    }
}

#[test]
fn reports_missing_humanoid_bones() {
    let mut builder = ModelBuilder::new();
    builder.add_node(Node::new("lonely"));
    let model = builder.finish().expect("one node is a model");
    let error =
        Body::from_anatomy(&model, &Anatomy::recognize(&model)).expect_err("nothing to map");
    assert!(error.missing.contains(&HumanBone::Hips));
    assert!(error.to_string().contains("leftUpperArm"));
}
