use animate_body::{HumanPose, LEFT_LEG, TORSO};
use animate_motion::{
    Goal, HumanMotion, Part, PartKey, PartValues, PoseSample, Slope, Slopes, Tag,
};
use rikka_model_core::glam::{EulerRot, Quat, Vec3};

fn bend(frame: u32, bent: f32, tag: Tag) -> PartKey {
    let mut muscles = vec![0.0; TORSO.len()];
    muscles[0] = bent;
    PartKey {
        part: Part::Torso,
        frame,
        tag,
        values: PartValues::Muscles(muscles),
        slopes: None,
    }
}

fn motion(keys: impl IntoIterator<Item = PartKey>) -> HumanMotion {
    let mut motion = HumanMotion::default();
    for key in keys {
        motion.insert(key);
    }
    motion
}

fn bent(motion: &HumanMotion, frame: f32) -> f32 {
    motion.sample(frame, &HumanPose::default()).pose.muscles[TORSO.start]
}

fn slope(motion: &HumanMotion, frame: f32) -> f32 {
    let step = 0.01;
    (bent(motion, frame + step) - bent(motion, frame - step)) / (2.0 * step)
}

fn frames(from: u32, to: u32) -> impl Iterator<Item = f32> {
    (from * 10..=to * 10).map(|tenth| tenth as f32 / 10.0)
}

#[test]
fn smooth_keys_are_monotone() {
    let keys = [(0, 0.0), (6, 0.1), (10, 0.9), (30, 1.0)];
    let motion = motion(keys.map(|(frame, value)| bend(frame, value, Tag::Smooth)));
    let mut previous = 0.0;
    for frame in frames(0, 30) {
        let value = bent(&motion, frame);
        assert!((0.0..=1.0).contains(&value), "{value} at {frame}");
        assert!(value >= previous - 1e-6, "falls back at {frame}");
        previous = value;
    }
    for (from, to) in keys.iter().zip(&keys[1..]) {
        for frame in frames(from.0, to.0) {
            let value = bent(&motion, frame);
            assert!(value >= from.1 - 1e-6 && value <= to.1 + 1e-6);
        }
    }
    assert!(slope(&motion, 0.02).abs() < 1e-3);
    assert!(slope(&motion, 29.98).abs() < 1e-3);
}

fn cubic(frame: f32) -> f32 {
    0.1 + 0.2 * frame - 0.03 * frame * frame + 0.001 * frame * frame * frame
}

fn cubic_slope(frame: f32) -> f32 {
    0.2 - 0.06 * frame + 0.003 * frame * frame
}

fn slope_at(frame: f32) -> Slope {
    Slope {
        arriving: cubic_slope(frame),
        leaving: cubic_slope(frame),
    }
}

#[test]
fn explicit_slopes_override_tags() {
    let keys = [(0, Tag::Step), (12, Tag::Ease), (30, Tag::Settle)].map(|(frame, tag)| {
        let at = frame as f32;
        PartKey {
            slopes: Some(Slopes {
                scalars: (0..TORSO.len())
                    .map(|channel| match channel {
                        0 => slope_at(at),
                        _ => Slope::default(),
                    })
                    .collect(),
                ..Slopes::default()
            }),
            ..bend(frame, cubic(at), tag)
        }
    });
    let motion = motion(keys);
    for frame in frames(0, 30) {
        let value = bent(&motion, frame);
        assert!((value - cubic(frame)).abs() < 1e-5, "{value} at {frame}");
    }
}

#[test]
fn linear_rotation_has_constant_speed() {
    let turns = [
        Quat::IDENTITY,
        Quat::from_rotation_y(90f32.to_radians()),
        Quat::from_euler(EulerRot::YXZ, 90f32.to_radians(), 60f32.to_radians(), 0.0),
    ];
    let motion = motion(turns.iter().enumerate().map(|(at, turn)| PartKey {
        part: Part::Root,
        frame: at as u32 * 10,
        tag: Tag::Linear,
        values: PartValues::Root {
            position: Vec3::ZERO,
            rotation: *turn,
        },
        slopes: None,
    }));
    let turn = |frame: f32| motion.sample(frame, &HumanPose::default()).pose.root_rot;
    for (span, degrees) in [(0, 9.0), (1, 6.0)] {
        for frame in span * 10..span * 10 + 10 {
            let swept = turn(frame as f32).angle_between(turn(frame as f32 + 1.0));
            assert!(
                (swept.to_degrees() - degrees).abs() < 0.05,
                "{} degrees at {frame}",
                swept.to_degrees()
            );
        }
    }
    let near = |one: Quat, two: Quat| one.dot(two).abs() > 1.0 - 1e-6;
    assert!(near(turn(10.0), turns[1]));
    assert!(near(turn(15.0), turns[1].slerp(turns[2], 0.5)));
}

fn leg(frame: u32, pin: bool) -> PartKey {
    PartKey {
        part: Part::LeftLeg,
        frame,
        tag: Tag::Smooth,
        values: PartValues::Limb {
            muscles: vec![0.0; LEFT_LEG.len()],
            pin,
            goal: Goal::default(),
        },
        slopes: None,
    }
}

#[test]
fn pin_weight_follows_span() {
    let motion = motion([leg(0, true), leg(10, true), leg(20, false), leg(30, false)]);
    let weight = |frame: f32| motion.sample(frame, &HumanPose::default()).pins[0].weight;
    let left = |frame: f32| motion.sample(frame, &HumanPose::default()).pins[2];
    assert_eq!(weight(5.0), 0.0, "the left arm has no keys");
    for frame in frames(0, 10) {
        assert_eq!(left(frame).weight, 1.0);
    }
    assert!((left(15.0).weight - 0.5).abs() < 1e-6);
    assert!(left(12.0).weight > left(18.0).weight);
    assert!(left(12.0).pin, "the flag steps");
    for frame in frames(20, 40) {
        assert_eq!(left(frame).weight, 0.0);
    }
}

#[test]
fn part_key_round_trips_sample() {
    let mut rest = HumanPose::default();
    rest.muscles[LEFT_LEG.start] = -0.3;
    let mut pose_sample = PoseSample::resting(&rest);
    for part in Part::ALL {
        let values = pose_sample.values(part);
        assert!(values.fits(part), "{part:?}");
        assert_eq!(values, PartValues::resting(part, &rest));
        pose_sample.set(part, &values);
    }
    assert_eq!(pose_sample, PoseSample::resting(&rest));
}
