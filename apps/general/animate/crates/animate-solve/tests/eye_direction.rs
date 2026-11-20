use animate_body::{Body, HumanBone, Side};
use animate_solve::EyeDirection;
use rikka_model_core::glam::Vec3;

use crate::support::{BODY, rest};

const AIM_MISS: f32 = 0.05;

#[test]
fn eye_direction_round_trips_through_pose() {
    let body: &Body = &BODY;
    let cases = [
        (
            EyeDirection {
                across: 1.0,
                up: 0.0,
            },
            Vec3::X,
        ),
        (
            EyeDirection {
                across: -1.0,
                up: 0.0,
            },
            Vec3::NEG_X,
        ),
        (
            EyeDirection {
                across: 0.0,
                up: 1.0,
            },
            Vec3::Y,
        ),
        (
            EyeDirection {
                across: 0.0,
                up: -1.0,
            },
            Vec3::NEG_Y,
        ),
        (
            EyeDirection {
                across: 0.5,
                up: 0.5,
            },
            Vec3::new(1.0, 1.0, 0.0),
        ),
    ];
    for limits in [true, false] {
        for (eye_direction, way) in cases {
            let mut pose = rest();
            for eye in Side::BOTH {
                eye_direction.apply(&mut pose, eye, limits);
            }
            let worlds = body.fk(&pose);
            let head = worlds.rotation(HumanBone::Head).inverse();
            for eye in Side::BOTH {
                let ahead = head * worlds.rotation(eye.eye()) * Vec3::Z;
                assert!(
                    ahead.dot(way) > 0.25,
                    "{eye_direction:?} turns the {eye:?} eye along {ahead} (limits {limits})"
                );
                let read = EyeDirection::from_pose(&pose, eye, limits);
                assert!(
                    (read.across - eye_direction.across).abs() < 1e-5
                        && (read.up - eye_direction.up).abs() < 1e-5,
                    "{eye_direction:?} reads back as {read:?} (limits {limits})"
                );
            }
        }
    }
}

#[test]
fn eyes_aim_at_target_within_reach() {
    let body: &Body = &BODY;
    let pose = rest();
    let standing = body.fk(&pose);
    let eyes = standing.position(HumanBone::LeftEye);

    let near = eyes + Vec3::new(0.05, 0.02, 1.0);
    let eye_direction =
        EyeDirection::aim(body, &standing, Side::Left, near, true).expect("miku has eyes");
    assert!(
        eye_direction.across > 0.0 && eye_direction.across < 1.0,
        "{eye_direction:?}"
    );
    assert!(
        eye_direction.up > 0.0 && eye_direction.up < 1.0,
        "{eye_direction:?}"
    );

    let beside = eyes + Vec3::new(3.0, 0.0, 0.1);
    let eye_direction =
        EyeDirection::aim(body, &standing, Side::Left, beside, true).expect("miku has eyes");
    assert!(
        (eye_direction.across - 1.0).abs() < 1e-5,
        "{eye_direction:?}"
    );
}

#[test]
fn eyes_look_exactly_at_targets_within_reach() {
    let body: &Body = &BODY;
    let pose = rest();
    let standing = body.fk(&pose);
    let aims = [
        Vec3::new(0.05, 0.02, 1.0),
        Vec3::new(-0.4, 0.0, 1.0),
        Vec3::new(0.0, 0.3, 1.0),
        Vec3::new(0.45, -0.35, 1.0),
        Vec3::new(-0.45, 0.4, 1.0),
    ];
    for eye in Side::BOTH {
        let from = standing.position(eye.eye());
        for aim in aims {
            let target = from + aim;
            let eye_direction =
                EyeDirection::aim(body, &standing, eye, target, false).expect("miku has eyes");
            let mut looking = pose;
            eye_direction.apply(&mut looking, eye, false);
            let worlds = body.fk(&looking);
            let ahead = worlds.rotation(eye.eye()) * Vec3::Z;
            let miss = ahead.angle_between(target - worlds.position(eye.eye()));
            assert!(
                miss.to_degrees() < AIM_MISS,
                "the {eye:?} eye misses {aim} by {} degrees",
                miss.to_degrees()
            );
        }
    }
}
