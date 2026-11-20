use std::sync::Arc;

use animate_body::{Body, Dof, HumanBone, LEFT_LEG, MUSCLE_COUNT, RIGHT_LEG, muscle_of, muscles};
use rikka_humanoid::Anatomy;
use rikka_mmd::{Puppet, Vmd};
use rikka_model_core::Instance;

use crate::support::{miku, read};
const DANCE: &[u8] =
    include_bytes!("../../../../../../crates/model/rikka-mmd/assets/mmd/motion.vmd");

const ANY_JUMP: f32 = 120.0;
const LEG_JUMP: f32 = 45.0;
const FOOT_REACH: f32 = 90.0;

fn danced() -> Vec<[f32; MUSCLE_COUNT]> {
    let model = Arc::new(miku());
    let body = Body::from_anatomy(&model, &Anatomy::recognize(&model)).expect("miku is human");
    let motion = read(DANCE, "motion.vmd")
        .extensions
        .remove::<Vmd>()
        .expect("a VMD motion");
    let animation = motion.animation(&model);
    let switches = motion.switches(&model);
    let mut instance = Instance::new(Arc::clone(&model));
    (0..=motion.last_frame())
        .map(|frame| {
            let time = frame as f32 / 30.0;
            instance.pose_mut().reset(&model);
            animation.apply(time, instance.pose_mut());
            if let Some(puppet) = instance.rig_as_mut::<Puppet>() {
                switches.apply(time, puppet);
            }
            instance.refresh();
            let pose = body.read(&instance);
            std::array::from_fn(|index| muscles()[index].angle(pose.muscles[index]).to_degrees())
        })
        .collect()
}

#[test]
fn dance_reads_continuously() {
    let frames = danced();
    let legs: Vec<usize> = LEFT_LEG.chain(RIGHT_LEG).collect();
    for (at, pair) in frames.windows(2).enumerate() {
        for (index, (before, after)) in pair[0].iter().zip(&pair[1]).enumerate() {
            let jump = (after - before).abs();
            let limit = match legs.contains(&index) {
                true => LEG_JUMP,
                false => ANY_JUMP,
            };
            assert!(
                jump < limit,
                "{} {:?} jumps {jump} degrees into frame {}",
                muscles()[index].bone,
                muscles()[index].dof,
                at + 1
            );
        }
    }
    for foot in [HumanBone::LeftFoot, HumanBone::RightFoot] {
        for dof in [Dof::Bend, Dof::Side] {
            let index = muscle_of(foot, dof).expect("a foot muscle");
            for (frame, angles) in frames.iter().enumerate() {
                assert!(
                    angles[index].abs() <= FOOT_REACH,
                    "{foot} {dof:?} turns {} degrees at frame {frame}",
                    angles[index]
                );
            }
        }
    }
    let knee = muscle_of(HumanBone::LeftLowerLeg, Dof::Bend).expect("a knee");
    let folded = frames.iter().map(|angles| angles[knee]).fold(0.0, f32::max);
    assert!(
        folded > 120.0,
        "the high knee lift folds only {folded} degrees"
    );
}
