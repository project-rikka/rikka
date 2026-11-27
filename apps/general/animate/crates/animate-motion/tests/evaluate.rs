use std::sync::{Arc, mpsc};
use std::time::Duration;

use animate_body::{HumanBone, HumanPose, LEFT_LEG, Side, TORSO};
use animate_motion::{
    Evaluator, Goal, HumanMotion, MotionBake, Part, PartKey, PoseSample, Tag, Written,
};
use rikka_model_core::glam::{Quat, Vec3};
use rikka_model_core::{NodeId, Pose};

use crate::support::{BODY, MODEL};

pub(crate) fn rest() -> HumanPose {
    *BODY.rest_pose()
}

pub(crate) fn eye(_: u32) -> Vec3 {
    Vec3::new(0.0, 1.5, 3.0)
}

pub(crate) fn keyed(
    part: Part,
    frame: u32,
    tag: Tag,
    pose: &HumanPose,
    edit: impl FnOnce(&mut PoseSample),
) -> PartKey {
    let mut pose_sample = PoseSample::resting(pose);
    edit(&mut pose_sample);
    PartKey {
        part,
        frame,
        tag,
        values: pose_sample.values(part),
        slopes: None,
    }
}

pub(crate) fn motion(keys: impl IntoIterator<Item = PartKey>) -> HumanMotion {
    let mut motion = HumanMotion::default();
    for key in keys {
        motion.insert(key);
    }
    motion
}

fn soles(pose: &HumanPose) -> Vec<f32> {
    let worlds = BODY.fk(pose);
    Side::BOTH
        .into_iter()
        .flat_map(|side| {
            let sole = BODY.sole(side);
            [
                worlds.point(side.foot(), sole.heel).y,
                worlds.point(side.toes(), sole.toe).y,
            ]
        })
        .collect()
}

#[test]
fn pinned_foot_holds_through_span() {
    let scale = BODY.scale();
    let ankle = BODY.fk(&rest()).joint(HumanBone::LeftFoot);
    let goal = Goal {
        position: (ankle.position + Vec3::new(0.0, 0.05, 0.03)) / scale,
        rotation: ankle.rotation,
    };
    let leg = |frame: u32, pin: bool, bend: f32| {
        keyed(Part::LeftLeg, frame, Tag::Smooth, &rest(), |pose_sample| {
            pose_sample.pose.muscles[LEFT_LEG.start] = bend;
            pose_sample.pins[2].pin = pin;
            pose_sample.pins[2].goal = goal;
        })
    };
    let root = |frame: u32, shift: Vec3| {
        keyed(Part::Root, frame, Tag::Smooth, &rest(), |pose_sample| {
            pose_sample.pose.root_pos += shift / scale;
        })
    };
    let motion = motion([
        leg(0, true, 0.0),
        leg(30, true, 0.5),
        leg(45, false, 0.6),
        root(0, Vec3::ZERO),
        root(30, Vec3::new(0.06, -0.04, 0.05)),
    ]);
    let mut evaluator = Evaluator::new(&BODY, &motion);
    let placed: Vec<Vec3> = (0..=45)
        .map(|frame| {
            let pose = evaluator.evaluate(frame, eye(frame));
            BODY.fk(&pose).position(HumanBone::LeftFoot)
        })
        .collect();
    for (frame, at) in placed.iter().enumerate().take(31) {
        let off = at.distance(goal.position * scale);
        assert!(
            off < 5e-4,
            "the pinned ankle drifts {off} m at frame {frame}"
        );
    }
    assert!(
        placed[45].distance(goal.position * scale) > 0.01,
        "the foot lets go"
    );
}

#[test]
fn evaluated_feet_stay_above_floor() {
    let scale = BODY.scale();
    let sink = |frame: u32, depth: f32| {
        keyed(Part::Root, frame, Tag::Smooth, &rest(), |pose_sample| {
            pose_sample.pose.root_pos.y -= depth / scale;
            pose_sample.pose.root_rot = Quat::from_rotation_x(depth);
        })
    };
    let motion = motion([sink(0, 0.0), sink(20, 0.25), sink(40, 0.05)]);
    let mut evaluator = Evaluator::new(&BODY, &motion);
    for frame in 0..=40 {
        let pose = evaluator.evaluate(frame, eye(frame));
        for height in soles(&pose) {
            assert!(
                height > -1e-4,
                "a sole sinks to {height} m at frame {frame}"
            );
        }
    }
}

pub(crate) fn dancing(torso: f32) -> HumanMotion {
    let mut keys = Vec::new();
    for (step, frame) in (0..=400).step_by(40).enumerate() {
        let swing = if step % 2 == 0 { 0.3 } else { -0.3 };
        keys.push(keyed(
            Part::Torso,
            frame,
            Tag::Smooth,
            &rest(),
            |pose_sample| {
                pose_sample.pose.muscles[TORSO.start + 2] = swing;
                if frame == 200 {
                    pose_sample.pose.muscles[TORSO.start] = torso;
                }
            },
        ));
        keys.push(keyed(
            Part::Root,
            frame,
            Tag::Ease,
            &rest(),
            |pose_sample| {
                pose_sample.pose.root_rot = Quat::from_rotation_y(swing);
            },
        ));
    }
    let mut motion = motion(keys);
    motion.settings.secondary = 0.5;
    motion
}

pub(crate) fn baked(motion_bake: MotionBake) -> MotionBake {
    motion_bake.wait();
    assert!(motion_bake.is_complete());
    motion_bake
}

pub(crate) fn same(one: &MotionBake, two: &MotionBake) {
    assert_eq!(one.frames(), two.frames());
    for (one, two) in poses(one).iter().zip(poses(two)) {
        for (a, b) in one.muscles.iter().zip(&two.muscles) {
            assert!((a - b).abs() < 1e-4, "{a} against {b}");
        }
        let apart = one.root_rot.inverse() * two.root_rot;
        let angle = 2.0 * apart.xyz().length().atan2(apart.w.abs());
        assert!(angle < 1e-4, "the root turns {angle} apart");
    }
}

pub(crate) fn poses(motion_bake: &MotionBake) -> Vec<HumanPose> {
    motion_bake.wait();
    (0..motion_bake.frames())
        .map(|frame| *motion_bake.baked_pose_at(frame).expect("a baked frame"))
        .collect()
}

#[test]
fn partial_rebake_matches_full_bake() {
    let body = Arc::clone(&BODY);
    let before = dancing(0.0);
    let after = dancing(0.6);
    let motion_bake = MotionBake::new(Arc::clone(&body), None, &before, eye);
    let baked = poses(&motion_bake);
    let dirty = after.dirty_since(&before).expect("the edit is seen");
    assert!(
        dirty.start > 0 && dirty.end < motion_bake.frames(),
        "{dirty:?}"
    );
    let rebaked = poses(&motion_bake.rebaked(&after, dirty.clone(), eye));
    same(
        &motion_bake.rebaked(&after, dirty.clone(), eye),
        &MotionBake::new(body, None, &after, eye),
    );
    assert_eq!(
        rebaked[..dirty.start as usize],
        baked[..dirty.start as usize]
    );
    assert_ne!(rebaked[200], baked[200]);
    assert_eq!(after.dirty_since(&after), None);
}

#[test]
fn baked_write_interpolates_frames() {
    let motion = dancing(0.0);
    let motion_bake = baked(MotionBake::new(Arc::clone(&BODY), None, &motion, eye));
    let pose_at = |seconds: f32| {
        let mut pose = Pose::rest(&MODEL);
        assert_eq!(motion_bake.write(seconds, &mut pose), Written::Baked);
        pose
    };
    let (before, after, between) = (
        pose_at(100.0 / 30.0),
        pose_at(101.0 / 30.0),
        pose_at(100.5 / 30.0),
    );
    for node in BODY.driven_nodes() {
        let (one, two, mixed) = (
            before.local(*node),
            after.local(*node),
            between.local(*node),
        );
        assert_eq!(
            mixed.translation,
            one.translation.lerp(two.translation, 0.5)
        );
        assert_eq!(mixed.rotation, one.rotation.slerp(two.rotation, 0.5));
        assert_eq!(mixed.scale, one.scale.lerp(two.scale, 0.5));
    }
    for node in (0..MODEL.nodes.len() as u32).map(NodeId) {
        let blended: Vec<f32> = before
            .weights(node)
            .iter()
            .zip(after.weights(node))
            .map(|(one, two)| one + (two - one) * 0.5)
            .collect();
        assert_eq!(between.weights(node), blended.as_slice());
    }
    let last = pose_at(1000.0);
    for node in BODY.driven_nodes() {
        assert_eq!(last.local(*node), pose_at(2000.0).local(*node));
    }
}

#[test]
fn unbaked_frames_are_estimated_until_the_bake_lands() {
    let motion = dancing(0.3);
    let motion_bake = MotionBake::new(Arc::clone(&BODY), None, &motion, eye);
    let estimate = |frame: u32| {
        motion
            .sample(frame as f32, BODY.rest_pose())
            .reached(&BODY, || eye(frame))
    };
    let (sender, landed) = mpsc::channel();
    let listener: Arc<dyn Fn() + Send + Sync> =
        Arc::new(move || sender.send(()).expect("the test listens"));
    while !motion_bake.is_complete() {
        for frame in 0..motion_bake.frames() {
            let pose = motion_bake.pose_at(frame);
            if !motion_bake.is_baked(frame) {
                assert_eq!(pose, estimate(frame));
            }
        }
        motion_bake.watch(&listener);
        landed
            .recv_timeout(Duration::from_secs(30))
            .expect("the bake progresses");
    }
    let mut pose = Pose::rest(&MODEL);
    assert_eq!(motion_bake.write(1.0, &mut pose), Written::Baked);
    assert!((0..motion_bake.frames()).all(|frame| motion_bake.is_baked(frame)));
}
