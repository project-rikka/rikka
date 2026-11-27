use std::sync::{Arc, LazyLock};

use animate_body::{Body, Faces, HumanBone, HumanPose, MUSCLE_COUNT};
use animate_motion::{Clip, Evaluator, Part, PartValues, Recipient, Retarget, Take, Target};
use rikka_humanoid::Anatomy;
use rikka_mmd::{Puppet, Vmd, VmdMorphFrame};
use rikka_model::glam::{Quat, Vec3};
use rikka_model::vrm::{
    AnimatedExpression, AnimatedLookAt, ExpressionName, ExpressionPreset, VrmAnimation,
};
use rikka_model::{
    Animation, Channel, Curve, Humanoid, Import, Instance, Interpolation, Model, ModelBuilder,
    Node, NodeId, Track, Transform, registry,
};

use crate::support::{BODY, dance, miku};

const AVATAR: &[u8] = include_bytes!("../../../../../../crates/model/rikka-vrm/assets/avatar.vrm");
const KNIGHT: &[u8] = include_bytes!("../../../../../../crates/model/rikka-fbx/assets/knight.fbx");
const MEAN_DEGREES: f32 = 5.0;
const MOST_DEGREES: f32 = 20.0;
const GAZE_YAW: f32 = 0.3;
const LOWERED: f32 = 1.3;

struct Figure {
    model: Arc<Model>,
    body: Body,
    faces: Faces,
}

impl Figure {
    fn read(name: &str, bytes: &[u8]) -> Self {
        let model = Arc::new(
            Import::named_bytes(name, bytes.to_vec(), registry())
                .run()
                .unwrap_or_else(|error| panic!("{name} reads: {error}")),
        );
        Self {
            body: Body::from_anatomy(&model, &Anatomy::recognize(&model))
                .unwrap_or_else(|error| panic!("{name} is human: {error}")),
            faces: Faces::from_model(&model),
            model,
        }
    }

    fn target(&self) -> Target<'_> {
        Target {
            model: &self.model,
            body: Some(&self.body),
            faces: &self.faces,
        }
    }
}

static AVATAR_FIGURE: LazyLock<Figure> = LazyLock::new(|| Figure::read("avatar.vrm", AVATAR));
static KNIGHT_FIGURE: LazyLock<Figure> = LazyLock::new(|| Figure::read("knight.fbx", KNIGHT));

fn evaluated(body: &Body, take: &Take, frames: impl Iterator<Item = u32>) -> Vec<HumanPose> {
    let mut quiet = take.human.clone().expect("a human motion");
    quiet.settings.secondary = 0.0;
    let mut evaluator = Evaluator::new(body, &quiet);
    frames
        .map(|frame| evaluator.evaluate(frame, Vec3::ZERO))
        .collect()
}

const COMPARED: [HumanBone; 9] = [
    HumanBone::Hips,
    HumanBone::Spine,
    HumanBone::Head,
    HumanBone::LeftUpperArm,
    HumanBone::RightUpperArm,
    HumanBone::LeftLowerArm,
    HumanBone::RightLowerArm,
    HumanBone::LeftUpperLeg,
    HumanBone::RightLowerLeg,
];

#[test]
fn vmd_on_vrm_follows_vmd_on_pmx() {
    let motion = dance();
    let avatar = &*AVATAR_FIGURE;
    let on_vrm = Take::from_vmd(avatar.target(), motion.clone());
    let on_pmx = Take::from_vmd(miku(true), motion.clone());
    let human = on_vrm.human.as_ref().expect("the dance moves the avatar");
    for part in Part::ALL {
        assert!(!human.parts().keys(&part).is_empty(), "{part:?}");
    }
    assert!(on_vrm.bones.is_empty() && on_vrm.switches.is_empty());
    let frames = || (0..=motion.last_frame()).step_by(7);
    let vrm_poses = evaluated(&avatar.body, &on_vrm, frames());
    let pmx_poses = evaluated(&BODY, &on_pmx, frames());
    for bone in COMPARED {
        let apart: Vec<f32> = vrm_poses
            .iter()
            .zip(&pmx_poses)
            .map(|(vrm, pmx)| {
                let one = avatar.body.fk(vrm).joint(bone).rotation;
                let two = BODY.fk(pmx).joint(bone).rotation;
                one.angle_between(two).to_degrees()
            })
            .collect();
        let mean = apart.iter().sum::<f32>() / apart.len() as f32;
        let most = apart.iter().copied().fold(0.0, f32::max);
        assert!(mean < MEAN_DEGREES, "{bone} turns {mean}° apart on average");
        assert!(most < MOST_DEGREES, "{bone} turns up to {most}° apart");
    }
}

#[test]
fn vmd_morphs_reach_vrm_expressions() {
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
            frame("no such morph", 5, 0.5),
        ],
        ..Vmd::default()
    };
    let spoken = Take::from_vmd(AVATAR_FIGURE.target(), lips);
    let keyed: Vec<&str> = spoken.morphs.keyed().map(String::as_str).collect();
    assert_eq!(keyed, ["aa", "blink", "no such morph"]);
    assert_eq!(spoken.morph_weight("aa", 20.0), 1.0);
}

const VRMA_BONES: &[(HumanBone, Option<usize>, [f32; 3])] = &[
    (HumanBone::Hips, None, [0.0, 0.95, 0.0]),
    (HumanBone::Spine, Some(0), [0.0, 0.1, 0.0]),
    (HumanBone::Chest, Some(1), [0.0, 0.15, 0.0]),
    (HumanBone::Neck, Some(2), [0.0, 0.25, 0.0]),
    (HumanBone::Head, Some(3), [0.0, 0.1, 0.0]),
    (HumanBone::LeftShoulder, Some(2), [0.04, 0.2, 0.0]),
    (HumanBone::LeftUpperArm, Some(5), [0.1, 0.0, 0.0]),
    (HumanBone::LeftLowerArm, Some(6), [0.26, 0.0, 0.0]),
    (HumanBone::LeftHand, Some(7), [0.24, 0.0, 0.0]),
    (HumanBone::RightShoulder, Some(2), [-0.04, 0.2, 0.0]),
    (HumanBone::RightUpperArm, Some(9), [-0.1, 0.0, 0.0]),
    (HumanBone::RightLowerArm, Some(10), [-0.26, 0.0, 0.0]),
    (HumanBone::RightHand, Some(11), [-0.24, 0.0, 0.0]),
    (HumanBone::LeftUpperLeg, Some(0), [0.09, -0.05, 0.0]),
    (HumanBone::LeftLowerLeg, Some(13), [0.0, -0.42, 0.0]),
    (HumanBone::LeftFoot, Some(14), [0.0, -0.4, 0.0]),
    (HumanBone::LeftToes, Some(15), [0.0, -0.06, 0.12]),
    (HumanBone::RightUpperLeg, Some(0), [-0.09, -0.05, 0.0]),
    (HumanBone::RightLowerLeg, Some(17), [0.0, -0.42, 0.0]),
    (HumanBone::RightFoot, Some(18), [0.0, -0.4, 0.0]),
    (HumanBone::RightToes, Some(19), [0.0, -0.06, 0.12]),
];

fn wave() -> Arc<Model> {
    let mut builder = ModelBuilder::new();
    let mut humanoid = Humanoid::default();
    let mut nodes: Vec<NodeId> = Vec::new();
    for (bone, parent, at) in VRMA_BONES {
        let node = builder.add_node(
            Node::new(bone.name()).transform(Transform::from_translation(Vec3::from(*at))),
        );
        if let Some(parent) = parent {
            builder.attach(nodes[*parent], node);
        }
        humanoid.set_bone(*bone, Some(node));
        nodes.push(node);
    }
    let happy = builder.add_node(Node::new("happy"));
    let gaze = builder.add_node(Node::new("gaze"));
    let times = vec![0.0, 1.0];
    let arm = humanoid.bone(HumanBone::LeftUpperArm).expect("a left arm");
    let mut animation = Animation::new("wave");
    animation.channels = vec![
        Channel {
            target: arm,
            track: Track::Rotation(Curve {
                times: times.clone(),
                values: vec![Quat::IDENTITY, Quat::from_rotation_z(-LOWERED)],
                interpolation: Interpolation::Linear,
            }),
        },
        Channel {
            target: happy,
            track: Track::Translation(Curve {
                times: times.clone(),
                values: vec![Vec3::ZERO, Vec3::X],
                interpolation: Interpolation::Linear,
            }),
        },
        Channel {
            target: gaze,
            track: Track::Rotation(Curve {
                times,
                values: vec![Quat::IDENTITY, Quat::from_rotation_y(GAZE_YAW)],
                interpolation: Interpolation::Linear,
            }),
        },
    ];
    builder.add_animation(animation);
    builder.extend_with(VrmAnimation {
        humanoid,
        expressions: vec![AnimatedExpression {
            name: ExpressionName::Preset(ExpressionPreset::Happy),
            node: happy,
        }],
        look_at: Some(AnimatedLookAt {
            node: Some(gaze),
            offset_from_head: None,
        }),
    });
    Arc::new(builder.finish().expect("the wave is a valid model"))
}

fn hand_below_shoulder(model: &Arc<Model>, body: &Body, pose: &HumanPose, side: HumanBone) -> f32 {
    let mut instance = Instance::new(Arc::clone(model));
    let pose_locals = body.locals(pose);
    for (node, local) in body.driven_nodes().iter().zip(pose_locals) {
        instance.pose_mut().set_local(*node, local);
    }
    if let Some(puppet) = instance.rig_as_mut::<Puppet>() {
        body.release(puppet);
    }
    instance.refresh();
    let at = |bone: HumanBone| {
        let node = body.mapping().node(bone).expect("a mapped bone");
        instance.world(node).w_axis.truncate()
    };
    let hand = match side {
        HumanBone::LeftUpperArm => HumanBone::LeftHand,
        _ => HumanBone::RightHand,
    };
    (at(side).y - at(hand).y) / body.scale()
}

#[test]
fn vrma_drives_pmx_and_vrm_with_expressions_and_gaze() {
    let clip = Clip::new(wave(), 0).expect("the wave has a clip");
    assert!(clip.is_human());
    for (figure, body) in [
        (miku(true), &**BODY),
        (AVATAR_FIGURE.target(), &AVATAR_FIGURE.body),
    ] {
        let take = Take::from_clip(figure, &clip);
        let human = take.human.as_ref().expect("the wave moves the figure");
        let last = take.last_frame();
        let poses = evaluated(body, &take, [0, last].into_iter());
        let (rest, lowered) = (
            hand_below_shoulder(figure.model, body, &poses[0], HumanBone::LeftUpperArm),
            hand_below_shoulder(figure.model, body, &poses[1], HumanBone::LeftUpperArm),
        );
        assert!(
            lowered > rest + 0.2,
            "the left hand drops: {rest} → {lowered}"
        );
        let still = hand_below_shoulder(figure.model, body, &poses[1], HumanBone::RightUpperArm);
        assert!(
            (still - rest).abs() < 0.1,
            "the right hand stays: {rest} → {still}"
        );
        let happy = figure.faces.adopt("happy");
        assert!(!happy.is_empty());
        for (morph, share) in happy {
            assert!(
                (take.morph_weight(morph, last as f32) - share).abs() < 1e-5,
                "{morph} is fully shown"
            );
        }
        let gaze = human.parts().keys(&Part::Gaze);
        let turned = gaze.iter().any(|key| match &key.values {
            PartValues::Gaze { eyes, .. } => eyes.iter().any(|eye| eye.abs() > 0.1),
            _ => false,
        });
        assert!(turned, "the eyes follow the look-at");
    }
}

#[test]
fn fbx_clip_drives_vrm_and_its_own_figure() {
    let knight = &*KNIGHT_FIGURE;
    let index = knight
        .model
        .animations
        .iter()
        .position(|animation| animation.name.contains("Unarmed_Melee_Attack_Punch_A"))
        .expect("the knight punches");
    let clip = Clip::new(Arc::clone(&knight.model), index).expect("a clip");
    assert!(clip.is_human());
    for target in [AVATAR_FIGURE.target(), knight.target(), miku(true)] {
        let take = Take::from_clip(target, &clip);
        let human = take.human.as_ref().expect("the punch moves the figure");
        assert!(
            !human.parts().keys(&Part::RightArm).is_empty()
                || !human.parts().keys(&Part::LeftArm).is_empty()
        );
        let body = target.body.expect("a body");
        for pose in evaluated(body, &take, 0..=take.last_frame()) {
            assert!(
                pose.root_pos.is_finite() && pose.muscles.iter().all(|muscle| muscle.is_finite())
            );
        }
    }
}

fn recipient(target: Target<'_>) -> Recipient {
    Recipient {
        model: Arc::clone(target.model),
        body: target.body.cloned().map(Arc::new),
        faces: Arc::new(target.faces.clone()),
    }
}

#[test]
fn vmd_preview_follows_imported_take() {
    let motion = dance();
    for (figure, body) in [
        (miku(true), &**BODY),
        (AVATAR_FIGURE.target(), &AVATAR_FIGURE.body),
    ] {
        let take = Take::from_vmd(figure, motion.clone());
        let human = take.human.as_ref().expect("the dance moves the figure");
        let mut retarget = Retarget::from_vmd(recipient(figure), &motion);
        assert_eq!(retarget.frames(), motion.last_frame() + 1);
        let mut instance = Instance::new(Arc::clone(figure.model));
        let frames: Vec<u32> = (0..retarget.frames()).step_by(7).collect();
        let apart: f32 = frames
            .iter()
            .map(|frame| {
                let preview = retarget
                    .write(*frame as f32 / 30.0, Vec3::ZERO, &mut instance)
                    .expect("the preview poses the figure");
                let keyed = human.sample(*frame as f32, body.rest_pose()).pose;
                preview
                    .muscles
                    .iter()
                    .zip(keyed.muscles)
                    .map(|(one, two)| (one - two).abs())
                    .sum::<f32>()
            })
            .sum();
        let mean = apart / (frames.len() * MUSCLE_COUNT) as f32;
        assert!(mean < 0.01, "the preview strays {mean} from the take");
    }
}

#[test]
fn clip_preview_lowers_the_hand() {
    let clip = Clip::new(wave(), 0).expect("the wave has a clip");
    let avatar = &*AVATAR_FIGURE;
    let mut retarget = Retarget::from_clip(recipient(avatar.target()), &clip);
    let last = retarget.frames() - 1;
    let [rest, lowered] = [0, last].map(|frame| {
        let pose = retarget
            .pose_at(frame as f32, Vec3::ZERO)
            .expect("the wave poses the avatar");
        hand_below_shoulder(&avatar.model, &avatar.body, &pose, HumanBone::LeftUpperArm)
    });
    assert!(
        lowered > rest + 0.2,
        "the left hand drops: {rest} → {lowered}"
    );
}
