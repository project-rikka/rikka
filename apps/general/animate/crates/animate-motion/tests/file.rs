use animate_body::{LEFT_ARM, Side};
use animate_hold::{Hold, HoldTarget, Holder, Holds};
use animate_motion::{
    Automation, CameraFile, CameraMotion, Error, Gaze, GazeMode, Goal, HumanMotion, Intensity,
    MorphKey, Part, PartKey, PartValues, Slope, Slopes, Tag, Take, TakeFile,
};
use rikka_mmd::{
    VmdBoneFrame, VmdCameraFrame, VmdCameraInterpolation, VmdCurve, VmdInterpolation,
    VmdPropertyFrame,
};
use rikka_model_core::glam::{Quat, Vec3};
use serde_json::{Value as Json, json};

const BENT: VmdCurve = VmdCurve {
    first: (64, 3),
    second: (90, 127),
};

fn figure() -> Take {
    let mut human = HumanMotion::default();
    human.settings = Automation {
        secondary: 0.6,
        blink: false,
        limits: true,
        seed: 41,
        fitting: false,
        jiggle: 0.75,
        intensity: Intensity {
            size: 1.5,
            snap: 0.5,
            torso: 1.25,
            head: 0.75,
            arms: 2.0,
            legs: 0.0,
            bounce: 1.75,
        },
    };
    let keys = [
        PartKey {
            part: Part::Root,
            frame: 0,
            tag: Tag::Settle,
            values: PartValues::Root {
                position: Vec3::new(0.1, 1.0, -0.25),
                rotation: Quat::from_rotation_y(0.3),
            },
            slopes: Some(Slopes {
                scalars: vec![
                    Slope {
                        arriving: 0.5,
                        leaving: -0.25,
                    };
                    3
                ],
                turn: Some(Slope {
                    arriving: 0.0,
                    leaving: 0.125,
                }),
            }),
        },
        PartKey {
            part: Part::LeftArm,
            frame: 12,
            tag: Tag::Hit,
            values: PartValues::Limb {
                muscles: (0..LEFT_ARM.len()).map(|at| at as f32 / 10.0).collect(),
                pin: true,
                goal: Goal {
                    position: Vec3::new(0.4, 1.2, 0.1),
                    rotation: Quat::from_rotation_x(-0.7),
                },
            },
            slopes: None,
        },
        PartKey {
            part: Part::Gaze,
            frame: 7,
            tag: Tag::Linear,
            values: PartValues::Gaze {
                eyes: vec![0.25, -0.5, 0.75, 0.1],
                gaze: Gaze {
                    mode: GazeMode::Camera,
                    target: Vec3::new(1.0, 1.5, 2.0),
                    weight: 0.8,
                    head_follow: 0.2,
                },
            },
            slopes: None,
        },
    ];
    for key in keys {
        human.insert(key);
    }
    Take {
        human: Some(human),
        bones: [VmdBoneFrame {
            bone: "髪".into(),
            frame: 5,
            translation: Vec3::new(0.0, 0.1, 0.2),
            rotation: Quat::from_rotation_z(0.4),
            interpolation: VmdInterpolation {
                y: BENT,
                ..VmdInterpolation::default()
            },
            physics_off: true,
        }]
        .into_iter()
        .collect(),
        switches: [VmdPropertyFrame {
            frame: 0,
            visible: true,
            ik: vec![("左足ＩＫ".into(), false)],
        }]
        .into_iter()
        .collect(),
        morphs: [
            MorphKey {
                morph: "あ".into(),
                frame: 9,
                weight: 0.3,
                curve: BENT,
            },
            MorphKey {
                morph: "にやり".into(),
                frame: 3,
                weight: 0.75,
                curve: VmdCurve::LINEAR,
            },
        ]
        .into_iter()
        .collect(),
        holds: sample_holds(),
    }
}

fn sample_holds() -> Holds {
    let mut holds = Holds::default();
    holds.lay(Hold {
        until: Some(40),
        ..Hold::new(
            Holder::Hand(Side::Right),
            HoldTarget {
                figure: Some("Rin".into()),
                at: Vec3::new(0.1, 0.6, 0.05),
                reach: 0.05,
            },
            12,
        )
    });
    holds
}

type Change = fn(&mut Json);

fn saved(take: &Take) -> Json {
    serde_json::to_value(TakeFile::from_take(take)).expect("a take serializes")
}

fn read(json: Json) -> Result<Take, Error> {
    serde_json::from_value::<TakeFile>(json)
        .expect("the file shape parses")
        .take()
}

fn edited(change: impl FnOnce(&mut Json)) -> Result<Take, Error> {
    let mut json = saved(&figure());
    change(&mut json);
    read(json)
}

fn renamed(json: &mut Json, from: &str, to: &str) {
    let text = json.to_string();
    assert!(text.contains(from), "{from} in {text}");
    *json = serde_json::from_str(&text.replacen(from, to, 1)).expect("still JSON");
}

fn key(json: &mut Json, track: usize) -> &mut Json {
    &mut json["human"]["tracks"][track]["keys"][0]
}

#[test]
fn figure_motion_round_trip() {
    for take in [
        figure(),
        Take {
            bones: figure().bones,
            switches: figure().switches,
            ..Take::default()
        },
    ] {
        assert_eq!(read(saved(&take)).ok(), Some(take));
    }
}

#[test]
fn camera_motion_round_trip() {
    let motion: CameraMotion = [VmdCameraFrame {
        frame: 30,
        distance: -4.5,
        target: Vec3::new(0.0, 1.2, 0.0),
        rotation: Vec3::new(0.1, -0.2, 0.0),
        interpolation: VmdCameraInterpolation {
            distance: BENT,
            ..VmdCameraInterpolation::default()
        },
        angle: 27,
        perspective: false,
    }]
    .into_iter()
    .collect();
    let json = serde_json::to_value(CameraFile::from_motion(&motion)).expect("serializes");
    let file: CameraFile = serde_json::from_value(json.clone()).expect("parses");
    assert_eq!(file.motion().ok(), Some(motion));
    let mut broken = json;
    broken["cameras"][0]["target"][1] = json!(1e39);
    let file: CameraFile = serde_json::from_value(broken).expect("an infinite target parses");
    assert!(matches!(
        file.motion(),
        Err(Error::Malformed { frame: 30, .. })
    ));
}

#[test]
fn rejects_unknown_tag() {
    assert!(matches!(
        edited(|json| renamed(json, "\"hit\"", "\"bounce\"")),
        Err(Error::UnknownTag(tag)) if tag == "bounce"
    ));
}

#[test]
fn rejects_duplicate_track_keys() {
    let doubled = edited(|json| {
        let morphs = json["morphs"].as_array_mut().expect("morph keys are saved");
        let first = morphs[0].clone();
        morphs.push(first);
    });
    assert!(matches!(
        doubled,
        Err(Error::Repeated { track, frame: 9 }) if track == "あ"
    ));
    let bones = edited(|json| {
        let bones = json["detail"]["bones"]
            .as_array_mut()
            .expect("bone keys are saved");
        let first = bones[0].clone();
        bones.push(first);
    });
    assert!(matches!(
        bones,
        Err(Error::Repeated { track, frame: 5 }) if track == "髪"
    ));
    let mut misplaced = saved(&figure());
    misplaced["detail"]["morphs"] = json!([{ "name": "あ", "frame": 0, "weight": 1.0 }]);
    assert!(serde_json::from_value::<TakeFile>(misplaced).is_err());
}

#[test]
fn rejects_key_values_of_other_part() {
    assert!(matches!(
        edited(|json| renamed(json, "\"leftArm\"", "\"rightLeg\"")),
        Err(Error::InvalidKey {
            part: Part::RightLeg,
            frame: 12
        })
    ));
    assert!(matches!(
        edited(|json| renamed(json, "\"gaze\"", "\"head\"")),
        Err(Error::InvalidKey {
            part: Part::Head,
            ..
        })
    ));
    let unturned = edited(|json| {
        let slopes = key(json, 0)["slopes"].as_object_mut().expect("root slopes");
        assert!(slopes.remove("turn").is_some());
    });
    assert!(matches!(
        unturned,
        Err(Error::InvalidKey {
            part: Part::Root,
            frame: 0
        })
    ));
}

#[test]
fn rejects_malformed_values() {
    let cases: [(&str, Change); 5] = [
        ("root", |json| {
            key(json, 0)["values"]["root"]["rotation"] = json!([0.0, 0.0, 0.0, 0.0]);
        }),
        ("leftArm", |json| {
            key(json, 1)["values"]["limb"]["goal"]["rotation"] = json!([0.0, 2.0, 0.0, 0.0]);
        }),
        ("gaze", |json| {
            key(json, 2)["values"]["gaze"]["headFollow"] = json!(1e39);
        }),
        ("髪", |json| {
            json["detail"]["bones"][0]["rotation"] = json!([0.5, 0.5, 0.5, 0.0]);
        }),
        ("あ", |json| json["morphs"][0]["weight"] = json!(1e39)),
    ];
    for (track, change) in cases {
        let read = edited(change);
        assert!(
            matches!(&read, Err(Error::Malformed { track: name, .. }) if name == track),
            "{track}: {read:?}"
        );
    }
}

#[test]
fn rejects_settings_out_of_range() {
    let cases: [(&str, Change); 3] = [
        ("jiggle", |json| {
            json["human"]["settings"]["jiggle"] = json!(1.5)
        }),
        ("secondary", |json| {
            json["human"]["settings"]["secondary"] = json!(-0.1)
        }),
        ("bounce", |json| {
            json["human"]["settings"]["intensity"]["bounce"] = json!(2.5);
        }),
    ];
    for (name, change) in cases {
        let read = edited(change);
        assert!(
            matches!(read, Err(Error::OutOfRange(found)) if found == name),
            "{name}"
        );
    }
}
