use std::sync::Arc;

use animate_body::{HumanBone, HumanPose};
use animate_motion::{Evaluator, Part, Take};
use animate_solve::ground;
use rikka_mmd::{Puppet, Vmd, VmdBoneFrame, VmdInterpolation, VmdMorphFrame};
use rikka_model_core::Instance;
use rikka_model_core::glam::{Quat, Vec3};

use crate::support::{BODY, FACES, MODEL, dance, miku};

fn played(motion: &Vmd, frames: u32) -> Vec<HumanPose> {
    let model = &*MODEL;
    let mut instance = Instance::new(Arc::clone(model));
    let animation = motion.animation(model);
    let switches = motion.switches(model);
    (0..frames)
        .map(|frame| {
            let time = frame as f32 / 30.0;
            instance.pose_mut().reset(model);
            animation.apply(time, instance.pose_mut());
            if let Some(puppet) = instance.rig_as_mut::<Puppet>() {
                switches.apply(time, puppet);
            }
            instance.refresh();
            BODY.read(&instance)
        })
        .collect()
}

fn with_hair(mut motion: Vmd) -> Vmd {
    motion.bones.push(VmdBoneFrame {
        bone: "左髮3".into(),
        frame: 12,
        translation: Vec3::ZERO,
        rotation: Quat::from_rotation_z(0.3),
        interpolation: VmdInterpolation::default(),
        physics_off: false,
    });
    motion
}

#[test]
fn import_reduces_keys_preserving_motion() {
    let motion = with_hair(dance());
    let frames = motion.last_frame() + 1;
    let take = Take::from_vmd(miku(true), motion.clone());
    let human = take.human.as_ref().expect("the dance drives the body");

    for part in Part::ALL {
        let keys = human.parts().keys(&part);
        assert!(!keys.is_empty(), "{part:?} is keyed");
        assert!(
            keys.len() * 3 < frames as usize,
            "{part:?} keeps {} keys of {frames} frames",
            keys.len()
        );
        assert!(keys.iter().all(|key| key.slopes.is_some()), "{part:?}");
    }
    assert!(!take.bones.keys("左髮3").is_empty());
    assert!(take.bones.keyed().all(|bone| {
        MODEL
            .find_node(bone)
            .is_none_or(|node| !BODY.mapping().is_driven(node))
    }));
    for frame in &motion.morphs {
        let morph = FACES.resolve(&frame.morph).unwrap_or(&frame.morph);
        let key = take
            .morphs
            .key(morph, frame.frame)
            .expect("every morph key");
        assert_eq!(key.weight, frame.weight, "{morph} at {}", frame.frame);
    }

    let original = played(&motion, frames);
    let mut quiet = human.clone();
    quiet.settings.secondary = 0.0;
    let mut evaluator = Evaluator::new(&BODY, &quiet);
    for (frame, desired) in original.iter().enumerate() {
        let evaluated = evaluator.evaluate(frame as u32, Vec3::ZERO);
        let mut grounded = *desired;
        ground(&BODY, &mut grounded);
        let (one, two) = (BODY.fk(&evaluated), BODY.fk(&grounded));
        for bone in [
            HumanBone::Hips,
            HumanBone::Head,
            HumanBone::LeftHand,
            HumanBone::RightHand,
            HumanBone::LeftFoot,
            HumanBone::RightFoot,
            HumanBone::LeftLowerArm,
            HumanBone::RightLowerArm,
        ] {
            let off = one.position(bone).distance(two.position(bone));
            assert!(off < 0.015, "{bone} strays {off} m at frame {frame}");
        }
    }
}

#[test]
fn morph_motion_merges_over_dance() {
    let frame = |morph: &str, frame: u32, weight: f32| VmdMorphFrame {
        morph: morph.into(),
        frame,
        weight,
    };
    let lips = Vmd {
        morphs: vec![
            frame("あ", 0, 0.0),
            frame("あ", 20, 1.0),
            frame("まばたき", 10, 1.0),
            frame(" まばたき ", 30, 0.0),
            frame("no such morph", 5, 0.5),
        ],
        ..Vmd::default()
    };
    let spoken = Take::from_vmd(miku(true), lips.clone());
    assert!(spoken.human.is_none(), "morphs alone move no body part");
    assert!(spoken.bones.is_empty());
    let keyed: Vec<&str> = spoken.morphs.keyed().map(String::as_str).collect();
    assert_eq!(keyed, ["no such morph", "あ", "まばたき"]);
    assert_eq!(spoken.morphs.keys("まばたき").len(), 2);
    assert!((spoken.morph_weight("あ", 5.0) - 0.25).abs() < 1e-6);
    assert!((spoken.morph_weight("まばたき", 20.0) - 0.5).abs() < 1e-6);
    assert_eq!(spoken.morph_weight("あ", 40.0), 1.0);
    assert_eq!(Take::from_vmd(miku(false), lips).morphs, spoken.morphs);

    let mut motion = with_hair(dance());
    motion.bones.retain(|frame| frame.frame < 60);
    motion.morphs.clear();
    let mut take = Take::from_vmd(miku(true), motion);
    let torso = take
        .human
        .as_ref()
        .map(|human| human.parts().keys(&Part::Torso).to_vec());
    take.merge(spoken.clone());
    let merged = take.human.as_ref().expect("the dance stays");
    assert_eq!(Some(merged.parts().keys(&Part::Torso).to_vec()), torso);
    assert_eq!(take.morphs, spoken.morphs);
    assert!(!take.bones.keys("左髮3").is_empty());
}
