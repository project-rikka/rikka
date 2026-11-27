use std::sync::{Arc, LazyLock};

use animate_body::{HumanBone, Limb};
use animate_fit::Physique;
use animate_motion::{HumanMotion, Intensity, MotionBake, Part, Tag};
use animate_solve::reach;
use rikka_humanoid::Anatomy;
use rikka_model_core::Pose;
use rikka_model_core::glam::{Quat, Vec3};
use rikka_physics_tailor::{ClothAnalysis, ClothSettings, Wearer};

use crate::evaluate::{baked, dancing, eye, keyed, motion, poses, rest, same};
use crate::support::{BODY, MODEL};

static PHYSIQUE: LazyLock<Arc<Physique>> = LazyLock::new(|| {
    let anatomy = Anatomy::recognize(&MODEL);
    let analysis =
        ClothAnalysis::new(&MODEL, &anatomy, &ClothSettings::new()).expect("miku analyses");
    let wearer = Wearer::from_model(&MODEL, &anatomy, &analysis).expect("miku has a skin");
    Arc::new(Physique::new(&BODY, &MODEL, &wearer))
});

fn reaching(torso: f32) -> HumanMotion {
    let mut motion = dancing(torso);
    let chest = BODY.fk(&rest()).position(HumanBone::UpperChest);
    for (step, frame) in (0..=400).step_by(40).enumerate() {
        let ahead = if step % 2 == 0 { 0.3 } else { 0.0 } * BODY.scale();
        let mut pose = rest();
        reach(
            &BODY,
            &mut pose,
            Limb::LeftArm,
            chest + Vec3::Z * ahead,
            None,
            1.0,
        );
        motion.insert(keyed(Part::LeftArm, frame, Tag::Smooth, &pose, |_| ()));
    }
    motion
}

fn plain(mut motion: HumanMotion) -> HumanMotion {
    motion.settings.fitting = false;
    motion.settings.jiggle = 0.0;
    motion
}

fn hand_travel(motion_bake: &MotionBake) -> f32 {
    let hands: Vec<Vec3> = poses(motion_bake)
        .iter()
        .map(|pose| BODY.fk(pose).position(HumanBone::LeftHand))
        .collect();
    hands.windows(2).map(|pair| pair[1].distance(pair[0])).sum()
}

#[test]
fn intensity_scales_motion_range() {
    let travel = |size: f32| {
        let mut motion = plain(reaching(0.0));
        motion.settings.intensity = Intensity {
            size,
            ..Intensity::default()
        };
        hand_travel(&MotionBake::new(Arc::clone(&BODY), None, &motion, eye))
    };
    let (calm, neutral, wide) = (travel(0.5), travel(1.0), travel(1.5));
    assert!(calm < neutral * 0.8, "{calm} against {neutral}");
    assert!(wide > neutral * 1.2, "{wide} against {neutral}");
}

#[test]
fn bust_jiggle_decays_after_stop() {
    assert!(!BODY.lobes().is_empty(), "miku's breasts are flesh");
    let turn = |frame: u32, angle: f32| {
        keyed(Part::Root, frame, Tag::Linear, &rest(), |pose_sample| {
            pose_sample.pose.root_rot = Quat::from_rotation_y(angle);
        })
    };
    let swung = |jiggle: f32| {
        let mut motion = motion([turn(0, 0.0), turn(6, 1.2), turn(60, 1.2)]);
        motion.settings.secondary = 0.0;
        motion.settings.jiggle = jiggle;
        let motion_bake = baked(MotionBake::new(Arc::clone(&BODY), None, &motion, eye));
        move |frame: u32| {
            let mut pose = Pose::rest(&MODEL);
            motion_bake.write(frame as f32 / 30.0, &mut pose);
            BODY.lobes()
                .iter()
                .map(|lobe| {
                    let rest = Pose::rest(&MODEL).local(lobe.node).rotation;
                    pose.local(lobe.node)
                        .rotation
                        .angle_between(rest)
                        .to_degrees()
                })
                .fold(0.0, f32::max)
        }
    };
    let bouncing = swung(0.8);
    assert!(bouncing(8) > 1.0, "the bust lags the turn: {}", bouncing(8));
    assert!(bouncing(80) < 0.05, "the bust settles: {}", bouncing(80));
    let still = swung(0.0);
    assert!((0..90).all(|frame| still(frame) == 0.0));
}

#[test]
fn styled_rebake_matches_full_bake() {
    let styles = [
        (
            true,
            0.6,
            Intensity {
                size: 1.4,
                snap: 1.2,
                ..Intensity::default()
            },
        ),
        (false, 1.0, Intensity::default()),
    ];
    for (fitting, jiggle, intensity) in styles {
        let styled = |torso: f32| {
            let mut motion = reaching(torso);
            motion.settings.fitting = fitting;
            motion.settings.jiggle = jiggle;
            motion.settings.intensity = intensity;
            motion
        };
        let (before, after) = (styled(0.0), styled(0.6));
        let physique = Some(Arc::clone(&PHYSIQUE));
        let motion_bake = MotionBake::new(Arc::clone(&BODY), physique.clone(), &before, eye);
        let unedited = poses(&motion_bake);
        let dirty = after.dirty_since(&before).expect("the edit is seen");
        let rebaked = baked(motion_bake.rebaked(&after, dirty, eye));
        let whole = baked(MotionBake::new(Arc::clone(&BODY), physique, &after, eye));
        same(&rebaked, &whole);
        assert_ne!(poses(&rebaked), unedited, "the rebake shows the edit");
        for frame in 0..whole.frames() {
            let lobes = |motion_bake: &MotionBake| {
                let mut pose = Pose::rest(&MODEL);
                motion_bake.write(frame as f32 / 30.0, &mut pose);
                BODY.lobes()
                    .iter()
                    .map(|lobe| pose.local(lobe.node).rotation)
                    .collect::<Vec<_>>()
            };
            for (one, two) in lobes(&rebaked).iter().zip(lobes(&whole)) {
                let apart = one.inverse() * two;
                let angle = 2.0 * apart.xyz().length().atan2(apart.w.abs());
                assert!(angle < 1e-4, "frame {frame}: the bust turns {angle} apart");
            }
        }
    }
}

#[test]
fn background_bake_matches_one_pass_rebake() {
    let motion = reaching(0.3);
    let motion_bake = MotionBake::new(Arc::clone(&BODY), Some(Arc::clone(&PHYSIQUE)), &motion, eye);
    let background = poses(&motion_bake);
    assert!(background.len() > 320, "the bake spans several pieces");
    let rebaked = baked(motion_bake.rebaked(&motion, 250..262, eye));
    assert_eq!(poses(&rebaked), background);
    for frame in 0..motion_bake.frames() {
        let written = |motion_bake: &MotionBake| {
            let mut pose = Pose::rest(&MODEL);
            motion_bake.write(frame as f32 / 30.0, &mut pose);
            pose
        };
        assert_eq!(written(&rebaked), written(&motion_bake), "frame {frame}");
    }
}

#[test]
fn fitting_removes_hand_from_chest() {
    let sunk = |fitting: bool, gap: f32| {
        let mut motion = plain(reaching(0.0));
        motion.settings.fitting = fitting;
        let motion_bake = baked(MotionBake::new(
            Arc::clone(&BODY),
            Some(Arc::clone(&PHYSIQUE)),
            &motion,
            eye,
        ));
        let worlds = BODY.fk(&motion_bake.pose_at(40));
        BODY.bulk().holds(
            &worlds,
            worlds.position(HumanBone::LeftHand),
            gap * BODY.scale(),
        )
    };
    assert!(
        sunk(false, -0.03),
        "unfitted, the hand sinks into the chest"
    );
    assert!(!sunk(true, 0.0), "fitted, the hand rests on it");
}
