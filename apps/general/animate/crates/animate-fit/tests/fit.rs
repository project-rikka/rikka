use animate_body::{HumanBone, HumanPose, Limb, Worlds};
use animate_solve::reach;
use rikka_model_core::glam::Vec3;

use crate::support::SAMPLE;

const FRAMES: usize = 161;
const HELD_AWAY: usize = 40;

fn hand_inside_chest_pose() -> Vec<HumanPose> {
    let body = &SAMPLE.body;
    let rest = *body.rest_pose();
    let chest = body.fk(&rest).position(HumanBone::UpperChest);
    let away = chest + Vec3::new(0.35, 0.0, 0.45) * body.scale();
    (0..FRAMES)
        .map(|frame| {
            let middle = (FRAMES / 2) as f32;
            let reaching = middle - HELD_AWAY as f32;
            let share = ((frame as f32 - middle).abs() / reaching).min(1.0);
            let mut pose = rest;
            reach(
                body,
                &mut pose,
                Limb::LeftArm,
                chest.lerp(away, share),
                None,
                1.0,
            );
            pose
        })
        .collect()
}

fn hand(worlds: &Worlds) -> Vec3 {
    worlds.position(HumanBone::LeftHand)
}

#[test]
fn fit_pushes_hand_out_of_chest() {
    let body = &SAMPLE.body;
    let scale = body.scale();
    let before = hand_inside_chest_pose();
    let unfitted: Vec<(HumanPose, [bool; 2])> =
        before.iter().map(|pose| (*pose, [false; 2])).collect();
    let after = SAMPLE.physique.fit(body, 0, &unfitted, 0..FRAMES);
    assert_eq!(
        after[0], before[0],
        "a hand far from the body is left alone"
    );

    let deepest = FRAMES / 2;
    let (was, now) = (body.fk(&before[deepest]), body.fk(&after[deepest]));
    let bulk = body.bulk();
    assert!(
        bulk.holds(&was, hand(&was), -0.03 * scale),
        "the hand starts inside"
    );
    assert!(
        !bulk.holds(&now, hand(&now), -0.01 * scale),
        "the hand is pulled out"
    );
    let chest = now.position(HumanBone::UpperChest);
    let facing = now.rotation(HumanBone::Hips) * Vec3::Z;
    assert!(
        (hand(&now) - chest).dot(facing) > 0.0,
        "the hand leaves through the front"
    );

    let jolt = |poses: &[HumanPose]| {
        let path: Vec<Vec3> = poses.iter().map(|pose| hand(&body.fk(pose))).collect();
        path.windows(3)
            .map(|three| (three[0] - 2.0 * three[1] + three[2]).length())
            .fold(0.0, f32::max)
    };
    assert!(
        jolt(&after) < 1.5 * jolt(&before),
        "the correction eases in and out without a jolt: {} against {}",
        jolt(&after),
        jolt(&before)
    );
}
