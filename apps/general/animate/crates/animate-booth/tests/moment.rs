use std::f32::consts::PI;
use std::sync::Arc;

use animate_body::{Body, Faces, HumanPose};
use animate_booth::{camera_frame, expressive_frame};
use animate_motion::{CameraMotion, Recipient, Retarget};
use rikka_model::ImportOptions;
use rikka_model::glam::{Quat, Vec3};
use rikka_model::humanoid::Anatomy;
use rikka_model::mmd::{Vmd, VmdCameraFrame, VmdCameraInterpolation};

const MOTION: &str = "crates/model/rikka-mmd/assets/mmd/motion.vmd";
const EYE: Vec3 = Vec3::new(0.0, 0.8, 3.6);

fn key(frame: u32) -> VmdCameraFrame {
    VmdCameraFrame {
        frame,
        distance: -3.0,
        target: Vec3::new(0.0, 1.0, 0.0),
        rotation: Vec3::ZERO,
        interpolation: VmdCameraInterpolation::default(),
        angle: 30,
        perspective: true,
    }
}

fn bent(pose: &HumanPose, rest: &HumanPose) -> f32 {
    pose.muscles
        .iter()
        .zip(&rest.muscles)
        .map(|(muscle, resting)| (muscle - resting).abs())
        .sum()
}

#[test]
fn camera_frame_is_middle_of_longest_cut() {
    let cases: [(&[u32], u32); 5] = [
        (&[], 0),
        (&[12], 12),
        (&[0, 100], 50),
        (&[0, 30, 31, 100, 101, 120], 65),
        (&[0, 60, 61, 70, 71, 200], 135),
    ];
    for (frames, expected) in cases {
        let motion: CameraMotion = frames.iter().copied().map(key).collect();
        assert_eq!(camera_frame(&motion), expected, "keys {frames:?}");
    }
}

#[test]
fn expressive_frame_skips_opening_and_bends_body() {
    let model = animate_mannequin::model();
    let anatomy = Anatomy::recognize(&model);
    let body = Arc::new(Body::from_anatomy(&model, &anatomy).expect("a body"));
    let bytes = std::fs::read(MOTION).expect("the sample motion");
    let vmd = Vmd::read(&bytes, &ImportOptions::default()).expect("a vmd");
    let mut retarget = Retarget::from_vmd(
        Recipient {
            faces: Arc::new(Faces::from_model(&model)),
            body: Some(Arc::clone(&body)),
            model,
        },
        &vmd,
    );
    let rest = *body.rest_pose();
    let last = retarget.frames() - 1;
    let mut pose_at = |frame: u32| retarget.pose_at(frame as f32, EYE);
    let frame = expressive_frame(last, &rest, &mut pose_at);
    assert!(last > 60, "sample motion lasts {last} frames");
    assert!((30..=last).contains(&frame), "frame {frame} of {last}");
    let chosen = bent(&pose_at(frame).expect("a pose"), &rest);
    let opening = bent(&pose_at(30).expect("a pose"), &rest);
    assert!(chosen >= opening, "chosen {chosen} opening {opening}");
}

#[test]
fn expressive_frame_passes_over_turned_back() {
    let rest = HumanPose::default();
    let posed = |bend: f32, turn: f32| {
        let mut pose = rest;
        pose.muscles.fill(bend);
        pose.root_rot = Quat::from_rotation_y(turn);
        pose
    };
    let frame = expressive_frame(120, &rest, |frame| match frame {
        60 => Some(posed(0.6, PI)),
        90 => Some(posed(0.4, 0.0)),
        _ => Some(rest),
    });
    assert_eq!(frame, 90);
}
