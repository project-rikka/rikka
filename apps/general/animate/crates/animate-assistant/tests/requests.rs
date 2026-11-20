use animate_assistant::{Extremity, Given, GraphOp, NodeRef, ToolRequest};
use animate_body::{HandPreset, Limb, Side};
use animate_cloth::Knob;
use animate_look::GraphRef;
use animate_motion::{GazeMode, Tag};
use animate_wind::WindSetting;
use rikka_mmd::VmdCurve;
use rikka_physics_tailor::Preset;
use rikka_shader_graph::NodeKind;
use serde_json::json;

use crate::support::ordered;

#[tokio::test]
async fn pose_reads_joint_degrees() {
    let Ok(ToolRequest::Pose { figure, keys }) = ordered(
        "Pose",
        json!({
            "figure": "miku",
            "keys": [{
                "frame": 12,
                "tag": "hit",
                "joints": { "leftUpperArm": { "bend": -70, "side": 15 } },
                "hands": { "right": "peace" },
                "expressions": { "happy": 0.8 },
                "reach": { "leftFoot": [0.1, 0.0, 0.2] },
                "pins": { "leftLeg": true },
                "gaze": {
                    "mode": "camera",
                    "headFollow": 0.3,
                    "eyes": { "both": [0.5, -0.2], "right": [0.1, 0.0] }
                }
            }]
        }),
    )
    .await
    else {
        panic!("a pose");
    };
    assert_eq!(figure, "miku");
    let key = &keys[0];
    assert_eq!(key.tag, Some(Tag::Hit));
    assert_eq!(key.joints.len(), 2);
    assert_eq!(key.hands, [(Side::Right, HandPreset::Peace)]);
    assert_eq!(key.reach[0].0, Extremity::Foot(Side::Left));
    assert_eq!(key.pins, [(Limb::LeftLeg, true)]);
    let gaze = key.gaze.expect("a gaze");
    assert_eq!(gaze.mode, Some(GazeMode::Camera));
    let eye_of = |side| gaze.eye(side).map(|eye| (eye.across, eye.up));
    assert_eq!(eye_of(Side::Left), Some((0.5, -0.2)));
    assert_eq!(eye_of(Side::Right), Some((0.1, 0.0)));
}

#[tokio::test]
async fn rejects_invalid_pose_with_choices() {
    let error = ordered(
        "Pose",
        json!({
            "figure": "miku",
            "keys": [{ "frame": 3, "joints": { "leftLowerArm": { "side": 10 } } }]
        }),
    )
    .await
    .expect_err("an elbow does not swing sideways");
    assert!(error.contains("bend, twist"), "{error}");
    let error = ordered(
        "Pose",
        json!({
            "figure": "miku",
            "keys": [{ "frame": 3, "joints": { "leftEye": { "bend": 10 } } }]
        }),
    )
    .await
    .expect_err("eyes follow the gaze");
    assert!(error.contains("frame 3"), "{error}");
    for eyes in [
        json!({ "both": [1.5, 0.0] }),
        json!({ "middle": [0.0, 0.0] }),
    ] {
        assert!(
            ordered(
                "Pose",
                json!({ "figure": "miku", "keys": [{ "frame": 0, "gaze": { "eyes": eyes } }] })
            )
            .await
            .is_err(),
            "{eyes} is refused"
        );
    }
    assert!(
        ordered(
            "Pose",
            json!({ "figure": "miku", "keys": [{ "frame": 0, "wiggle": 1 }] })
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn graph_edits_parse_in_order() {
    let Ok(ToolRequest::EditGraph(edits)) = ordered(
        "EditGraph",
        json!({
            "graph": "world",
            "edits": [
                { "add": "world.light", "name": "key", "values": { "intensity": 3 } },
                { "connect": ["key", "light"], "to": [0, "lights"] },
                { "set": [4, "color"], "value": [1.0, 0.8, 0.6] },
                { "unset": [4, "azimuth"] },
                { "disconnect": [0, "fog"] },
                { "remove": 7 }
            ]
        }),
    )
    .await
    else {
        panic!("graph edits");
    };
    assert_eq!(edits.graph, GraphRef::World);
    assert_eq!(edits.ops.len(), 6);
    let GraphOp::Add {
        name, kind, values, ..
    } = &edits.ops[0]
    else {
        panic!("an add first");
    };
    assert_eq!(name.as_deref(), Some("key"));
    assert_eq!(*kind, NodeKind::Extern("world.light".into()));
    assert_eq!(values, &[("intensity".into(), Given::Numbers(vec![3.0]))]);
    assert!(matches!(
        &edits.ops[1],
        GraphOp::Connect { from, .. } if from.node == NodeRef::Named("key".into())
    ));
}

#[tokio::test]
async fn rejects_malformed_look_requests() {
    for (tool, input) in [
        (
            "EditGraph",
            json!({ "graph": "sky", "edits": [{ "remove": 1 }] }),
        ),
        ("EditGraph", json!({ "graph": "world", "edits": [] })),
        (
            "EditGraph",
            json!({ "graph": "world", "edits": [{ "connect": [1, "light"] }] }),
        ),
        (
            "EditGraph",
            json!({ "graph": "world", "edits": [{ "spin": 1 }] }),
        ),
        ("Materials", json!({})),
        (
            "LookKeys",
            json!({ "set": [{ "target": { "graph": "world" }, "frame": 0 }] }),
        ),
        ("ResetLook", json!({ "world": "dusk" })),
        ("Nodes", json!({ "graph": "everything" })),
    ] {
        assert!(
            ordered(tool, input.clone()).await.is_err(),
            "{tool} {input}"
        );
    }
    let error = ordered(
        "EditGraph",
        json!({ "graph": "world", "edits": [{ "remove": 1 }, { "set": [1, "x"] }] }),
    )
    .await
    .expect_err("a set needs a value");
    assert!(error.contains("edit 1"), "{error}");
}

#[tokio::test]
async fn wind_keys_by_setting_and_frame() {
    let Ok(ToolRequest::SetWind(edit)) = ordered(
        "SetWind",
        json!({
            "resting": { "turbulence": 0.6 },
            "keys": [{ "frame": 30, "values": { "speed": 6, "yaw": 90 }, "curve": "linear" }],
            "clear": ["gust"]
        }),
    )
    .await
    else {
        panic!("a wind change");
    };
    assert_eq!(edit.resting, [(WindSetting::Turbulence, 0.6)]);
    assert_eq!(edit.keys[0].frame, 30);
    assert!(edit.keys[0].values.contains(&(WindSetting::Speed, 6.0)));
    assert_eq!(edit.keys[0].curve, Some(VmdCurve::LINEAR));
    assert_eq!(edit.clear, [WindSetting::Gust]);

    let error = ordered("SetWind", json!({ "resting": { "speed": 99 } }))
        .await
        .expect_err("too fast");
    assert!(error.contains("0 to 30"), "{error}");
    let error = ordered("SetWind", json!({ "clear": ["breeze"] }))
        .await
        .expect_err("unknown");
    assert!(error.contains("`turbulence`"), "{error}");
}

#[tokio::test]
async fn cloth_piece_order_parses() {
    let Ok(ToolRequest::SetCloth {
        figure,
        feel,
        pieces,
    }) = ordered(
        "SetCloth",
        json!({
            "figure": "miku",
            "feel": { "response": 0.9 },
            "pieces": [{
                "piece": "skirt",
                "preset": "cape",
                "enabled": true,
                "feel": { "weight": 0.8, "sway": 0.2 }
            }]
        }),
    )
    .await
    else {
        panic!("a cloth change");
    };
    assert_eq!(figure, "miku");
    assert_eq!(feel, vec![(Knob::Response, 0.9)]);
    assert_eq!(pieces[0].preset, Some(Preset::Cape));
    assert_eq!(pieces[0].enabled, Some(true));
    assert!(pieces[0].feel.contains(&(Knob::Weight, 0.8)));

    let error = ordered(
        "SetCloth",
        json!({ "figure": "miku", "pieces": [{ "piece": "skirt", "preset": "kimono" }] }),
    )
    .await
    .expect_err("no such preset");
    assert!(error.contains("long_hair"), "{error}");
    let error = ordered(
        "SetCloth",
        json!({ "figure": "miku", "pieces": [{ "piece": "skirt", "feel": { "weight": 3 } }] }),
    )
    .await
    .expect_err("out of range");
    assert!(error.contains("outside 0 to 1"), "{error}");
}
