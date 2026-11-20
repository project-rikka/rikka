use animate_body::{Body, Dof, HumanBone, Side, muscle_of};
use animate_solve::{Handle, Pin, rotate, twist};
use rikka_model_core::glam::{Quat, Vec3};

use crate::support::{BODY, changed, feet_stay, rest};
#[test]
fn rotating_centre_keeps_feet_planted() {
    let body: &Body = &BODY;
    let pose = rest();
    let result = rotate(
        body,
        &pose,
        Handle::Centre,
        Quat::from_rotation_y(0.3),
        &[],
        true,
    );
    feet_stay(body, &pose, &result, "Centre turned");
    assert!(result.root_rot.angle_between(pose.root_rot) > 0.25);
}

#[test]
fn rotating_upper_body_keeps_hips() {
    let body: &Body = &BODY;
    let pose = rest();
    let turn = Quat::from_rotation_y(0.4);
    let result = rotate(body, &pose, Handle::UpperBody, turn, &[], true);
    assert_eq!(result.root_pos, pose.root_pos);
    assert_eq!(result.root_rot, pose.root_rot);
    let (before, after) = (body.fk(&pose), body.fk(&result));
    let top = body
        .parent(HumanBone::Neck)
        .expect("a chest under the neck");
    let miss = after
        .rotation(top)
        .angle_between(turn * before.rotation(top));
    assert!(miss < 0.02, "the chest missed the turn by {miss} rad");
    for bone in [HumanBone::Spine, HumanBone::Chest] {
        let twist = muscle_of(bone, Dof::Twist).expect("a twisting spine");
        assert!(
            changed(&pose, &result).contains(&twist),
            "{bone:?} takes a share"
        );
    }
}

#[test]
fn rotating_lower_body_keeps_torso_and_feet() {
    let body: &Body = &BODY;
    let pose = rest();
    let result = rotate(
        body,
        &pose,
        Handle::LowerBody,
        Quat::from_rotation_y(0.25),
        &[],
        true,
    );
    let (before, after) = (body.fk(&pose), body.fk(&result));
    assert!(result.root_rot.angle_between(pose.root_rot) > 0.1);
    let spine_turn = after
        .rotation(HumanBone::Spine)
        .angle_between(before.rotation(HumanBone::Spine));
    assert!(spine_turn < 0.01, "the upper body turned {spine_turn} rad");
    feet_stay(body, &pose, &result, "LowerBody turned");
}

#[test]
fn rotation_stops_at_joint_limit() {
    let body: &Body = &BODY;
    let pose = rest();
    let worlds = body.fk(&pose);
    let lower = HumanBone::LeftLowerArm;
    let bend =
        worlds.rotation(HumanBone::LeftUpperArm) * body.axis(lower, Dof::Bend).expect("an elbow");
    let result = rotate(
        body,
        &pose,
        Handle::Forearm(Side::Left),
        Quat::from_axis_angle(bend, 3.0),
        &[],
        true,
    );
    assert!(
        result
            .muscles
            .iter()
            .all(|muscle| muscle.abs() <= 1.0 + 1e-6)
    );
    let index = muscle_of(lower, Dof::Bend).expect("an elbow");
    assert!(result.muscles[index] > 0.9, "{}", result.muscles[index]);
    assert!(Vec3::ZERO.distance(result.root_pos - pose.root_pos) < 1e-6);
}

#[test]
fn twist_moves_only_grabbed_part() {
    let body: &Body = &BODY;
    let pose = rest();
    let twist_of = |bone: HumanBone| muscle_of(bone, Dof::Twist).expect("a twisting bone");
    for (handle, twisted) in [
        (
            Handle::UpperArm(Side::Left),
            vec![twist_of(HumanBone::LeftUpperArm)],
        ),
        (
            Handle::Hand(Side::Right),
            vec![twist_of(HumanBone::RightLowerArm)],
        ),
        (
            Handle::Head,
            vec![twist_of(HumanBone::Neck), twist_of(HumanBone::Head)],
        ),
    ] {
        let result = twist(body, &pose, handle, 0.4, &[], true);
        assert_eq!(changed(&pose, &result), twisted, "{handle:?}");
        assert_eq!(result.root_rot, pose.root_rot, "{handle:?}");
    }
    let centre_result = twist(body, &pose, Handle::Centre, 0.4, &[], true);
    let yaw = centre_result.root_rot * pose.root_rot.inverse();
    assert!((yaw.angle_between(Quat::from_rotation_y(0.4))) < 1e-4);
    feet_stay(body, &pose, &centre_result, "Centre twisted");
}

#[test]
fn pin_survives_rotation() {
    let body: &Body = &BODY;
    let pose = rest();
    let pin = Pin::from_pose(body, &pose, Side::Right.arm());
    let result = rotate(
        body,
        &pose,
        Handle::UpperBody,
        Quat::from_rotation_y(0.3),
        &[pin],
        true,
    );
    let hand = body.fk(&result).joint(HumanBone::RightHand);
    assert!(hand.position.distance(pin.position) < 0.001);
}
