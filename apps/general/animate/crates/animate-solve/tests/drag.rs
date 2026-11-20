use std::sync::Arc;

use animate_body::{Body, Dof, HEAD, HumanBone, HumanPose, Limb, Side, TORSO, muscle_of, muscles};
use animate_solve::{DragScope, DragSession, Handle, Pin};
use rikka_model_core::glam::{Quat, Vec3};

use crate::support::{BODY, changed, feet_stay};

fn within_limits(pose: &HumanPose, limits: bool) -> bool {
    pose.muscles.iter().zip(muscles()).all(|(value, muscle)| {
        let (low, high) = muscle.bounds(limits);
        (low - 1e-6..=high + 1e-6).contains(value)
    })
}

fn torso_turn(pose: &HumanPose, after: &HumanPose) -> f32 {
    TORSO
        .map(|index| {
            let muscle = muscles()[index];
            (muscle.angle(after.muscles[index]) - muscle.angle(pose.muscles[index])).abs()
        })
        .fold(0.0, f32::max)
}

fn root_tilt(pose: &HumanPose, after: &HumanPose) -> f32 {
    after.root_rot.angle_between(pose.root_rot)
}

type Spread = fn(&HumanPose, &HumanPose) -> f32;

fn dragged(
    body: &Arc<Body>,
    pose: HumanPose,
    handle: Handle,
    pins: &[Pin],
    scope: DragScope,
    travel: Vec3,
) -> HumanPose {
    let mut session = DragSession::begin(Arc::clone(body), pose, handle, pins, true);
    for step in 1..=10 {
        session.update(travel * (step as f32 / 10.0), scope);
    }
    session.end()
}

#[test]
fn hand_drag_keeps_elbow_outside_body() {
    const STEPS: usize = 40;
    let body = Arc::clone(&BODY);
    let pose = *body.rest_pose();
    let worlds = body.fk(&pose);
    let scale = body.scale();
    let hips = worlds.position(HumanBone::Hips);
    let head = worlds.position(HumanBone::Head);
    let shoulder = worlds.position(HumanBone::LeftUpperArm);
    let other = worlds.position(HumanBone::RightUpperArm);
    let handle = Handle::Hand(Side::Left);
    let start = handle.point(&body, &worlds);
    for (name, target) in [
        (
            "down at the side",
            shoulder + Vec3::new(0.03, -0.4, 0.0) * scale,
        ),
        ("before the belly", hips + Vec3::new(0.0, 0.2, 0.2) * scale),
        (
            "before the chest",
            other + Vec3::new(0.0, -0.05, 0.2) * scale,
        ),
        ("at the cheek", head + Vec3::new(0.06, -0.03, 0.14) * scale),
        ("on the hip", hips + Vec3::new(0.18, 0.05, 0.0) * scale),
        ("raised high", shoulder + Vec3::new(0.1, 0.36, 0.1) * scale),
    ] {
        let mut session = DragSession::begin(Arc::clone(&body), pose, handle, &[], true);
        let mut elbow = worlds.position(HumanBone::LeftLowerArm);
        let mut current = pose;
        for step in 1..=STEPS {
            current = session.update(
                (target - start) * (step as f32 / STEPS as f32),
                DragScope::Limb,
            );
            let now = body.fk(&current).position(HumanBone::LeftLowerArm);
            let leap = now.distance(elbow) / (start.distance(target) / STEPS as f32);
            assert!(leap < 10.0, "{name}: the elbow leaps {leap} hand steps");
            elbow = now;
        }
        let after = body.fk(&current);
        let miss = handle.point(&body, &after).distance(target);
        assert!(miss < 0.002 * scale, "{name}: the hand stays {miss} short");
        assert!(within_limits(&current, true), "{name}");
        assert!(
            !body.bulk().holds(&after, elbow, 0.0),
            "{name}: the elbow sinks into the body"
        );
    }
}

#[test]
fn pinned_limbs_stay_during_drag() {
    let body = Arc::clone(&BODY);
    let pose = *body.rest_pose();
    let pins = [
        Pin::from_pose(&body, &pose, Limb::LeftLeg),
        Pin::from_pose(&body, &pose, Limb::RightLeg),
        Pin::from_pose(&body, &pose, Limb::RightArm),
    ];
    for (handle, travel) in [
        (Handle::Hand(Side::Left), Vec3::new(0.15, 0.3, 0.25)),
        (Handle::UpperBody, Vec3::new(0.1, -0.05, 0.15)),
        (Handle::Forearm(Side::Right), Vec3::new(0.0, 0.0, 0.1)),
    ] {
        let mut session = DragSession::begin(Arc::clone(&body), pose, handle, &pins, true);
        for step in 1..=5 {
            let result = session.update(travel * (step as f32 / 5.0), DragScope::FullBody);
            let worlds = body.fk(&result);
            for pin in &pins {
                let end = pin.limb.chain()[2];
                let moved = worlds.position(end).distance(pin.position);
                assert!(moved < 0.001, "{handle:?}: {:?} slid {moved} m", pin.limb);
                let angle = worlds.rotation(end).angle_between(pin.rotation);
                assert!(
                    angle < 0.01,
                    "{handle:?}: {:?} turned {angle} rad",
                    pin.limb
                );
            }
        }
    }
}

#[test]
fn full_body_engaged_only_on_request() {
    let body = Arc::clone(&BODY);
    let pose = *body.rest_pose();
    let rows: [(Handle, Vec3, Spread); 2] = [
        (
            Handle::Hand(Side::Left),
            Vec3::new(1.2, 0.0, 0.3),
            torso_turn,
        ),
        (Handle::UpperBody, Vec3::new(0.0, -0.2, 1.0), root_tilt),
    ];
    for (handle, away, spread) in rows {
        let start = handle.point(&body, &body.fk(&pose));
        let target = start + away;
        let miss_of =
            |candidate: &HumanPose| handle.point(&body, &body.fk(candidate)).distance(target);
        let limb_only = dragged(&body, pose, handle, &[], DragScope::Limb, away);
        let carried = dragged(&body, pose, handle, &[], DragScope::FullBody, away);
        assert!(
            spread(&pose, &limb_only) < 1e-5,
            "{handle:?} moved the body"
        );
        assert!(spread(&pose, &carried) > 0.05, "{handle:?} left the body");
        assert_eq!(limb_only.root_pos, pose.root_pos, "{handle:?}");
        assert_eq!(limb_only.root_rot, pose.root_rot, "{handle:?}");
        assert!(miss_of(&carried) < miss_of(&limb_only), "{handle:?}");
        if let Handle::Hand(_) = handle {
            let shoulder = body.fk(&pose).position(HumanBone::LeftUpperArm);
            let grip = handle.point(&body, &body.fk(&limb_only));
            let off = (grip - shoulder)
                .normalize()
                .angle_between((target - shoulder).normalize());
            assert!(off < 0.02, "the arm stops {off} rad off the pointer");
        }
    }
}

#[test]
fn drags_respect_muscle_limits() {
    let body = Arc::clone(&BODY);
    let pose = *body.rest_pose();
    for handle in Handle::ALL {
        for limits in [true, false] {
            for travel in [Vec3::new(-0.6, 0.9, -0.8), Vec3::new(0.7, -0.5, 0.9)] {
                let mut session = DragSession::begin(Arc::clone(&body), pose, handle, &[], limits);
                for step in 1..=6 {
                    let result = session.update(travel * (step as f32 / 6.0), DragScope::FullBody);
                    assert!(
                        within_limits(&result, limits),
                        "{handle:?} keeps limits {limits}"
                    );
                }
            }
        }
    }
}

#[test]
fn pose_beyond_limits_is_not_snapped() {
    let body = Arc::clone(&BODY);
    let mut pose = *body.rest_pose();
    let spine = muscle_of(HumanBone::Spine, Dof::Bend).expect("a bending spine");
    let elbow = muscle_of(HumanBone::LeftLowerArm, Dof::Bend).expect("an elbow");
    pose.muscles[spine] = 1.3;
    pose.muscles[elbow] = -0.6;
    let mut session = DragSession::begin(
        Arc::clone(&body),
        pose,
        Handle::Hand(Side::Right),
        &[],
        true,
    );
    let result = session.update(Vec3::new(0.0, 0.05, 0.05), DragScope::Limb);
    assert_eq!(result.muscles[spine], pose.muscles[spine]);
    assert_eq!(result.muscles[elbow], pose.muscles[elbow]);
}

#[test]
fn drag_session_is_deterministic() {
    let body = Arc::clone(&BODY);
    let pose = *body.rest_pose();
    let handle = Handle::Head;
    let travel = Vec3::new(0.04, 0.0, 0.08);
    let run = || {
        let mut session = DragSession::begin(Arc::clone(&body), pose, handle, &[], true);
        (0..4)
            .map(|_| session.update(travel, DragScope::FullBody))
            .collect::<Vec<_>>()
    };
    let one = run();
    assert_eq!(one, run());
    let change = |one: &HumanPose, other: &HumanPose| {
        one.muscles
            .iter()
            .zip(other.muscles)
            .map(|(one, other)| (one - other).abs())
            .fold(0.0, f32::max)
    };
    assert!(change(&one[3], &one[2]) <= change(&one[1], &one[0]) + 1e-6);
    assert!(change(&one[3], &one[2]) < 0.01);
}

#[test]
fn straight_leg_reaches_head_height() {
    const STEPS: usize = 30;
    let body = Arc::clone(&BODY);
    let pose = *body.rest_pose();
    let handle = Handle::Foot(Side::Left);
    let worlds = body.fk(&pose);
    let hip = worlds.position(HumanBone::LeftUpperLeg);
    let start = handle.point(&body, &worlds);
    for (name, degrees, axis) in [
        ("split to the side", 166.0f32, Vec3::Z),
        ("kick to the front", 150.0, Vec3::NEG_X),
    ] {
        let mut session = DragSession::begin(Arc::clone(&body), pose, handle, &[], true);
        let mut current = pose;
        let mut target = start;
        for step in 1..=STEPS {
            let turn =
                Quat::from_axis_angle(axis, (degrees * step as f32 / STEPS as f32).to_radians());
            target = hip + turn * (start - hip);
            current = session.update(target - start, DragScope::Limb);
        }
        let miss = handle.point(&body, &body.fk(&current)).distance(target);
        assert!(miss < 0.01, "{name}: the foot stays {miss} m short");
        assert!(within_limits(&current, true), "{name}");
    }
}

#[test]
fn feet_stay_planted_during_body_drag() {
    let body = Arc::clone(&BODY);
    let pose = *body.rest_pose();
    let scale = body.scale();
    let crouched = dragged(
        &body,
        pose,
        Handle::Centre,
        &[],
        DragScope::Limb,
        Vec3::NEG_Y * 0.12 * scale,
    );
    feet_stay(&body, &pose, &crouched, "Centre down");
    let (before, after) = (body.fk(&pose), body.fk(&crouched));
    assert!(after.position(HumanBone::Hips).y < before.position(HumanBone::Hips).y - 0.1 * scale);
    for (handle, travel, scope) in [
        (
            Handle::Centre,
            Vec3::new(0.05, -0.02, 0.04),
            DragScope::Limb,
        ),
        (
            Handle::LowerBody,
            Vec3::new(0.03, 0.0, 0.03),
            DragScope::Limb,
        ),
        (
            Handle::UpperBody,
            Vec3::new(0.0, -0.4, 0.5),
            DragScope::FullBody,
        ),
    ] {
        let result = dragged(&body, crouched, handle, &[], scope, travel * scale);
        feet_stay(&body, &crouched, &result, &format!("{handle:?}"));
        assert_ne!(result, crouched, "{handle:?}");
    }
}

#[test]
fn centre_carries_feet_with_full_body() {
    let body = Arc::clone(&BODY);
    let pose = *body.rest_pose();
    let travel = Vec3::new(0.1, 0.3, 0.0) * body.scale();
    let feet = |candidate: &HumanPose| {
        let worlds = body.fk(candidate);
        Side::BOTH.map(|side| worlds.position(side.foot()))
    };
    let before = feet(&pose);
    let mut session = DragSession::begin(Arc::clone(&body), pose, Handle::Centre, &[], true);
    let carried = session.update(travel, DragScope::FullBody);
    for (now, was) in feet(&carried).into_iter().zip(before) {
        assert!(now.distance(was + travel) < 0.001, "the feet stay behind");
    }
    let planted = session.update(travel * 1.2, DragScope::Limb);
    for (now, was) in feet(&planted).into_iter().zip(feet(&carried)) {
        assert!(
            now.distance(was) < 0.001,
            "the feet leap {}",
            now.distance(was)
        );
    }
}

#[test]
fn swing_keeps_marker_under_pointer() {
    let body = Arc::clone(&BODY);
    let pose = *body.rest_pose();
    let worlds = body.fk(&pose);
    let elbow_bend = worlds.rotation(HumanBone::LeftUpperArm)
        * body
            .axis(HumanBone::LeftLowerArm, Dof::Bend)
            .expect("an elbow");
    let neck = match body.has(HumanBone::Neck) {
        true => HumanBone::Neck,
        false => HumanBone::Head,
    };
    for (handle, pivot, axis, angle) in [
        (
            Handle::Shoulder(Side::Left),
            HumanBone::LeftShoulder,
            Vec3::Z,
            0.2,
        ),
        (
            Handle::UpperArm(Side::Left),
            HumanBone::LeftUpperArm,
            Vec3::X,
            0.5,
        ),
        (
            Handle::Forearm(Side::Left),
            HumanBone::LeftLowerArm,
            elbow_bend,
            0.8,
        ),
        (Handle::Toes(Side::Left), HumanBone::LeftFoot, Vec3::X, -0.3),
        (Handle::UpperBody, HumanBone::Spine, Vec3::X, 0.3),
        (Handle::Head, neck, Vec3::X, 0.3),
        (Handle::LowerBody, HumanBone::Hips, Vec3::Z, 0.15),
    ] {
        let grip = handle.point(&body, &worlds);
        let centre = worlds.position(pivot);
        let target = centre + Quat::from_axis_angle(axis, angle) * (grip - centre);
        let result = dragged(&body, pose, handle, &[], DragScope::Limb, target - grip);
        let miss = handle.point(&body, &body.fk(&result)).distance(target);
        let reach = grip.distance(centre);
        assert!(
            miss < 0.02 * reach,
            "{handle:?} leaves its marker {miss} from the pointer"
        );
    }
}

#[test]
fn lifted_foot_follows_shin() {
    let body = Arc::clone(&BODY);
    let pose = *body.rest_pose();
    let scale = body.scale();
    let handle = Handle::Foot(Side::Left);
    let [_, shin, foot] = Limb::LeftLeg.chain();
    let tilt = |candidate: &HumanPose| {
        let worlds = body.fk(candidate);
        worlds.rotation(foot).angle_between(worlds.rotation(shin))
    };
    let lifted = dragged(
        &body,
        pose,
        handle,
        &[],
        DragScope::Limb,
        Vec3::new(0.0, 0.4, 0.3) * scale,
    );
    let tilt_change = (tilt(&lifted) - tilt(&pose)).abs();
    assert!(
        tilt_change < 0.15,
        "the lifted foot turned {tilt_change} rad against the shin"
    );
    let slid = dragged(
        &body,
        pose,
        handle,
        &[],
        DragScope::Limb,
        Vec3::new(0.1, 0.0, 0.08) * scale,
    );
    let angle = body
        .fk(&slid)
        .rotation(foot)
        .angle_between(body.fk(&pose).rotation(foot));
    assert!(angle < 0.01, "the sliding foot turned {angle} rad");
}

#[test]
fn torso_and_head_drags_never_twist() {
    let body = Arc::clone(&BODY);
    let pose = *body.rest_pose();
    let twists: Vec<usize> = TORSO
        .chain(HEAD)
        .filter(|index| muscles()[*index].dof == Dof::Twist)
        .collect();
    for handle in [Handle::UpperBody, Handle::Head] {
        let result = dragged(
            &body,
            pose,
            handle,
            &[],
            DragScope::FullBody,
            Vec3::new(0.15, -0.05, 0.1),
        );
        for index in &twists {
            assert_eq!(result.muscles[*index], pose.muscles[*index], "{handle:?}");
        }
        assert!(!changed(&pose, &result).is_empty());
    }
}
