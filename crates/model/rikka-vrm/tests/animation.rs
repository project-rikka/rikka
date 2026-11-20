use std::sync::Arc;

use rikka_model_core::glam::Quat;
use rikka_model_core::{Error, HumanBone, Import, Model, Pose, Registry};
use rikka_vrm::{ExpressionName, ExpressionPreset, VrmAnimation, VrmAnimationImporter};
use serde_json::{Value, json};

const YAW: f32 = 0.3;

const BONES: &[(&str, Option<usize>, [f32; 3])] = &[
    ("hips", None, [0.0, 1.0, 0.0]),
    ("spine", Some(0), [0.0, 0.1, 0.0]),
    ("head", Some(1), [0.0, 0.5, 0.0]),
    ("leftUpperLeg", Some(0), [0.1, -0.05, 0.0]),
    ("leftLowerLeg", Some(3), [0.0, -0.45, 0.0]),
    ("leftFoot", Some(4), [0.0, -0.42, 0.0]),
    ("rightUpperLeg", Some(0), [-0.1, -0.05, 0.0]),
    ("rightLowerLeg", Some(6), [0.0, -0.45, 0.0]),
    ("rightFoot", Some(7), [0.0, -0.42, 0.0]),
    ("leftUpperArm", Some(1), [0.15, 0.35, 0.0]),
    ("leftLowerArm", Some(9), [0.27, 0.0, 0.0]),
    ("leftHand", Some(10), [0.26, 0.0, 0.0]),
    ("rightUpperArm", Some(1), [-0.15, 0.35, 0.0]),
    ("rightLowerArm", Some(12), [-0.27, 0.0, 0.0]),
    ("rightHand", Some(13), [-0.26, 0.0, 0.0]),
    ("leftEye", Some(2), [0.03, 0.08, 0.08]),
];

struct Buffer {
    bytes: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl Buffer {
    fn accessor(&mut self, values: &[f32], kind: &str, count: usize) -> usize {
        let offset = self.bytes.len();
        for value in values {
            self.bytes.extend_from_slice(&value.to_le_bytes());
        }
        self.views.push(json!({
            "buffer": 0,
            "byteOffset": offset,
            "byteLength": values.len() * 4,
        }));
        let mut accessor = json!({
            "bufferView": self.views.len() - 1,
            "componentType": 5126,
            "count": count,
            "type": kind,
        });
        if kind == "SCALAR" {
            accessor["min"] = json!([values[0]]);
            accessor["max"] = json!([values[values.len() - 1]]);
        }
        self.accessors.push(accessor);
        self.accessors.len() - 1
    }
}

fn glb(document: &Value, binary: &[u8]) -> Vec<u8> {
    let mut json = serde_json::to_vec(document).expect("a JSON document");
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let mut binary = binary.to_vec();
    while !binary.len().is_multiple_of(4) {
        binary.push(0);
    }
    let length = 12 + 8 + json.len() + 8 + binary.len();
    let mut bytes = Vec::with_capacity(length);
    bytes.extend_from_slice(b"glTF");
    bytes.extend_from_slice(&2u32.to_le_bytes());
    bytes.extend_from_slice(&(length as u32).to_le_bytes());
    bytes.extend_from_slice(&(json.len() as u32).to_le_bytes());
    bytes.extend_from_slice(b"JSON");
    bytes.extend_from_slice(&json);
    bytes.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    bytes.extend_from_slice(b"BIN\0");
    bytes.extend_from_slice(&binary);
    bytes
}

fn dance(spec_version: &str, extended: bool) -> Vec<u8> {
    let mut nodes: Vec<Value> = BONES
        .iter()
        .map(|(name, _, at)| json!({ "name": name, "translation": at }))
        .collect();
    for (index, (_, parent, _)) in BONES.iter().enumerate() {
        if let Some(parent) = parent {
            let children = nodes[*parent]
                .as_object_mut()
                .expect("a node")
                .entry("children")
                .or_insert_with(|| json!([]));
            children
                .as_array_mut()
                .expect("children")
                .push(json!(index));
        }
    }
    let happy = nodes.len();
    nodes.push(json!({ "name": "happy" }));
    let smirk = nodes.len();
    nodes.push(json!({ "name": "smirk" }));
    let look_up = nodes.len();
    nodes.push(json!({ "name": "lookUp" }));
    let gaze = nodes.len();
    nodes.push(json!({ "name": "gaze" }));

    let mut buffer = Buffer {
        bytes: Vec::new(),
        views: Vec::new(),
        accessors: Vec::new(),
    };
    let times = buffer.accessor(&[0.0, 1.0], "SCALAR", 2);
    let weights = buffer.accessor(&[0.0, 0.0, 0.0, 1.5, 0.0, 0.0], "VEC3", 2);
    let turned = Quat::from_rotation_y(YAW);
    let turns = buffer.accessor(
        &[0.0, 0.0, 0.0, 1.0, turned.x, turned.y, turned.z, turned.w],
        "VEC4",
        2,
    );
    let human_bones: serde_json::Map<String, Value> = BONES
        .iter()
        .enumerate()
        .map(|(index, (name, _, _))| ((*name).to_owned(), json!({ "node": index })))
        .collect();
    let mut document = json!({
        "asset": { "version": "2.0" },
        "scene": 0,
        "scenes": [{ "nodes": [0, happy, smirk, look_up, gaze] }],
        "nodes": nodes,
        "buffers": [{ "byteLength": buffer.bytes.len() }],
        "bufferViews": buffer.views,
        "accessors": buffer.accessors,
        "animations": [{
            "channels": [
                { "sampler": 0, "target": { "node": happy, "path": "translation" } },
                { "sampler": 1, "target": { "node": gaze, "path": "rotation" } },
                { "sampler": 1, "target": { "node": 0, "path": "rotation" } },
            ],
            "samplers": [
                { "input": times, "output": weights },
                { "input": times, "output": turns },
            ],
        }],
    });
    if extended {
        document["extensionsUsed"] = json!(["VRMC_vrm_animation"]);
        document["extensions"] = json!({
            "VRMC_vrm_animation": {
                "specVersion": spec_version,
                "humanoid": { "humanBones": human_bones },
                "expressions": {
                    "preset": {
                        "happy": { "node": happy },
                        "lookUp": { "node": look_up },
                    },
                    "custom": { "smirk": { "node": smirk } },
                },
                "lookAt": { "node": gaze, "offsetFromHeadBone": [0.0, 0.06, 0.0] },
            }
        });
    }
    glb(&document, &buffer.bytes)
}

fn read(bytes: Vec<u8>) -> Result<Model, Error> {
    Import::named_bytes(
        "dance.vrma",
        bytes,
        Arc::new(Registry::new().with_importer(VrmAnimationImporter)),
    )
    .run()
}

#[test]
fn vrma_maps_humanoid_expressions_and_gaze() {
    let model = read(dance("1.0", true)).expect("the animation is read");
    let animation = model
        .extensions
        .get::<VrmAnimation>()
        .expect("a VRM animation");
    assert_eq!(animation.humanoid.missing().count(), 0);
    assert_eq!(animation.humanoid.bone(HumanBone::LeftEye), None);
    assert_eq!(
        animation
            .expressions
            .iter()
            .map(|expression| expression.name.clone())
            .collect::<Vec<_>>(),
        [
            ExpressionName::Preset(ExpressionPreset::Happy),
            ExpressionName::Custom("smirk".to_owned()),
        ]
    );
    assert_eq!(
        model.rig_facts().humanoid.as_ref(),
        Some(&animation.humanoid)
    );

    let mut pose = Pose::rest(&model);
    model.animations[0].apply(1.0, &mut pose);
    let weights: Vec<(String, f32)> = animation
        .expression_weights(&pose)
        .map(|(name, weight)| (name.as_str().to_owned(), weight))
        .collect();
    assert_eq!(
        weights,
        [("happy".to_owned(), 1.0), ("smirk".to_owned(), 0.0)]
    );
    let gaze = animation.gaze(&pose).expect("a gaze");
    assert!((gaze.yaw - YAW).abs() < 1e-5, "{gaze:?}");
    assert!(gaze.pitch.abs() < 1e-5, "{gaze:?}");
}

#[test]
fn vrma_without_its_extension_or_version_is_refused() {
    for (bytes, refused) in [
        (dance("1.0", false), "damaged"),
        (dance("2.0", true), "version"),
    ] {
        match (read(bytes), refused) {
            (Err(Error::Damaged(_)), "damaged")
            | (Err(Error::UnsupportedVersion { .. }), "version") => {}
            (other, _) => panic!("expected a {refused} refusal, got {other:?}"),
        }
    }
}
