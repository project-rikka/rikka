use animate_body::{Body, HumanBone, HumanPose, Limb, Side};
use animate_solve::{elbow, ground, look, reach};
use rikka_model_core::glam::{Quat, Vec3};

use crate::support::{BODY, rest};

#[test]
fn elbow_bends_toward_pole() {
    let root = Vec3::ZERO;
    let goal = Vec3::new(0.0, -1.0, 0.0);
    let bent = elbow(root, goal, 0.8, 0.6, Vec3::Z);
    assert!((bent.distance(root) - 0.8).abs() < 1e-5);
    assert!((bent.distance(goal) - 0.6).abs() < 1e-5);
    assert!(bent.z > 0.0);
    assert!(bent.x.abs() < 1e-6);

    let far = Vec3::new(0.0, -3.0, 0.0);
    let straight = elbow(root, far, 0.8, 0.6, Vec3::Z);
    assert!(straight.distance(Vec3::new(0.0, -0.8, 0.0)) < 1e-4);
}

#[test]
fn reaches_goal_within_millimetre() {
    let body: &Body = &BODY;
    for (limb, swing, fold) in [
        (Limb::LeftArm, Quat::from_rotation_y(-0.6), 0.7),
        (Limb::RightArm, Quat::from_rotation_z(-0.5), 0.8),
        (Limb::LeftLeg, Quat::from_rotation_x(-0.7), 0.75),
        (Limb::RightLeg, Quat::from_rotation_x(0.3), 0.9),
    ] {
        let mut pose = rest();
        let [root, _, end] = limb.chain();
        let worlds = body.fk(&pose);
        let start = worlds.joint(end);
        let shoulder = worlds.position(root);
        let goal = shoulder + swing * (start.position - shoulder) * fold;
        let turn = match limb.is_leg() {
            true => Quat::from_rotation_x(0.25) * start.rotation,
            false => Quat::from_rotation_y(0.3) * start.rotation,
        };
        reach(body, &mut pose, limb, goal, Some(turn), 1.0);
        let reached = body.fk(&pose).joint(end);
        assert!(
            reached.position.distance(goal) < 1e-3,
            "{limb:?} ends {} m away",
            reached.position.distance(goal)
        );
        assert!(
            reached.rotation.angle_between(turn).to_degrees() < 3.0,
            "{limb:?} is turned {} degrees off",
            reached.rotation.angle_between(turn).to_degrees()
        );
    }
}

#[test]
fn unreachable_goal_straightens_limb() {
    let body: &Body = &BODY;
    let mut pose = rest();
    let worlds = body.fk(&pose);
    let shoulder = worlds.position(HumanBone::RightUpperArm);
    let length =
        body.offset(HumanBone::RightLowerArm).length() + body.offset(HumanBone::RightHand).length();
    let toward = Vec3::new(-0.6, 0.3, 0.75).normalize();
    reach(
        body,
        &mut pose,
        Limb::RightArm,
        shoulder + toward * 3.0,
        None,
        1.0,
    );
    let wrist = body.fk(&pose).position(HumanBone::RightHand);
    assert!(((wrist - shoulder).length() - length).abs() < 1e-3);
    assert!((wrist - shoulder).normalize().dot(toward) > 0.9999);
}

fn soles(pose: &HumanPose) -> Vec<f32> {
    let body: &Body = &BODY;
    let worlds = body.fk(pose);
    Side::BOTH
        .into_iter()
        .flat_map(|side| {
            let sole = body.sole(side);
            [
                worlds.point(side.foot(), sole.heel).y,
                worlds.point(side.toes(), sole.toe).y,
            ]
        })
        .collect()
}

#[test]
fn solved_feet_stay_above_floor() {
    let body: &Body = &BODY;
    for sink in [0.05, 0.12, 0.6] {
        let mut pose = rest();
        pose.root_pos.y -= sink / body.scale();
        pose.root_rot = Quat::from_rotation_x(0.1) * Quat::from_rotation_y(0.4);
        assert!(soles(&pose).iter().any(|height| *height < -0.01));
        ground(body, &mut pose);
        for height in soles(&pose) {
            assert!(
                height > -1e-4,
                "a sole stays {height} m under after sinking {sink}"
            );
        }
    }
    let mut standing = rest();
    standing.root_pos.y += 0.1 / body.scale();
    let before = standing;
    ground(body, &mut standing);
    assert_eq!(standing, before, "feet in the air are left alone");
}

fn forward(pose: &HumanPose, bone: HumanBone) -> Vec3 {
    BODY.fk(pose).rotation(bone) * Vec3::Z
}

#[test]
fn gaze_turns_eyes_and_optionally_head() {
    let body: &Body = &BODY;
    let head = body.fk(&rest()).position(HumanBone::Head);
    let target = head + Vec3::new(1.5, 0.1, 1.0);

    let mut glancing = rest();
    look(body, &mut glancing, target, 1.0, 0.0);
    for eye in [HumanBone::LeftEye, HumanBone::RightEye] {
        assert!(
            forward(&glancing, eye).x > forward(&rest(), eye).x + 0.2,
            "{eye} turns left"
        );
    }
    assert!(forward(&glancing, HumanBone::Head).distance(forward(&rest(), HumanBone::Head)) < 1e-5);

    let mut turning = rest();
    look(body, &mut turning, target, 1.0, 1.0);
    assert!(forward(&turning, HumanBone::Head).x > forward(&rest(), HumanBone::Head).x + 0.3);
    let eye = body.fk(&turning).position(HumanBone::LeftEye);
    let miss = forward(&turning, HumanBone::LeftEye).angle_between(target - eye);
    assert!(
        miss.to_degrees() < 3.0,
        "the eye misses by {} degrees",
        miss.to_degrees()
    );
}

#[test]
fn reaching_current_position_is_identity() {
    let body: &Body = &BODY;
    let mut bent = rest();
    for (index, value) in [
        (23, 0.3),
        (25, -0.2),
        (26, 0.5),
        (27, 0.3),
        (82, 0.4),
        (79, 0.2),
    ] {
        bent.muscles[index] = value;
    }
    for start in [rest(), bent] {
        for limb in Limb::ALL {
            let mut pose = start;
            let end = body.fk(&pose).joint(limb.chain()[2]);
            reach(body, &mut pose, limb, end.position, Some(end.rotation), 1.0);
            for index in limb.muscles() {
                assert!(
                    (pose.muscles[index] - start.muscles[index]).abs() < 1e-3,
                    "{limb:?} muscle {index} moves from {} to {}",
                    start.muscles[index],
                    pose.muscles[index]
                );
            }
        }
    }
}
