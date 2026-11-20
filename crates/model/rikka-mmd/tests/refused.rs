use std::sync::Arc;

use rikka_mmd::VmdImporter;
use rikka_model_core::{Error, Import, ImportOptions, Limit, Limits, Model, Registry, Result};

const BENT: [u8; 4] = [10, 100, 100, 10];
const STRAIGHT: [u8; 4] = [20, 20, 107, 107];

fn bone_key(bone: &str, frame: u32, curve: [u8; 4]) -> Vec<u8> {
    let mut key = vec![0; 15];
    key[..bone.len()].copy_from_slice(bone.as_bytes());
    key.extend(frame.to_le_bytes());
    key.extend([0.0_f32; 3].iter().flat_map(|value| value.to_le_bytes()));
    key.extend(
        [0.0_f32, 0.0, 0.0, 1.0]
            .iter()
            .flat_map(|value| value.to_le_bytes()),
    );
    let mut interpolation = [0; 64];
    for curve_slot in 0..4 {
        for (parameter, value) in curve.iter().enumerate() {
            interpolation[(curve_slot * 4 + parameter) * 4] = *value;
        }
    }
    key.extend(interpolation);
    key
}

fn motion(keys: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = vec![0; 50];
    bytes[..25].copy_from_slice(b"Vocaloid Motion Data 0002");
    bytes.extend((keys.len() as u32).to_le_bytes());
    bytes.extend(keys.iter().flatten());
    bytes.extend(0_u32.to_le_bytes());
    bytes
}

fn read(bytes: Vec<u8>, limits: Limits) -> Result<Model> {
    Import::named_bytes(
        "motion.vmd",
        bytes,
        Arc::new(Registry::new().with_importer(VmdImporter)),
    )
    .options(ImportOptions::new().limits(limits))
    .run()
}

fn over_keyframes(result: Result<Model>) -> bool {
    matches!(
        result,
        Err(Error::LimitsExceeded {
            limit: Limit::Keyframes,
            ..
        })
    )
}

#[test]
fn motion_longer_than_the_keyframe_limit_is_rejected() {
    let distant = motion(&[
        bone_key("a", 0, STRAIGHT),
        bone_key("a", 4_000_000_000, BENT),
    ]);
    assert!(over_keyframes(read(distant, Limits::default())));
}

#[test]
fn bent_spans_count_every_sampled_frame() {
    let limits = Limits {
        max_keyframes: 1000,
        ..Limits::default()
    };
    let keys = |curve| {
        motion(&[
            bone_key("a", 0, STRAIGHT),
            bone_key("a", 600, curve),
            bone_key("b", 0, STRAIGHT),
            bone_key("b", 600, curve),
        ])
    };
    assert!(over_keyframes(read(keys(BENT), limits)));
    let straight = read(keys(STRAIGHT), limits).expect("four keys fit");
    assert_eq!(straight.animations.len(), 1);
}
