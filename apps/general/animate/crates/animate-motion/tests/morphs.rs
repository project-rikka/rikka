use std::sync::Arc;

use animate_body::{Faces, MorphWeights};
use animate_motion::{HumanMotion, MorphKey, MorphPlayback, Take};
use rikka_mmd::{FRAMES_PER_SECOND, Pmx, VmdCurve};
use rikka_model_core::Instance;

use crate::support::MODEL;

const BENT: VmdCurve = VmdCurve {
    first: (64, 3),
    second: (90, 127),
};

fn key(morph: &str, frame: u32, weight: f32, curve: VmdCurve) -> MorphKey {
    MorphKey {
        morph: morph.into(),
        frame,
        weight,
        curve,
    }
}

fn morph_playback(take: Take) -> MorphPlayback {
    MorphPlayback::new(Arc::new(Faces::from_model(&MODEL)), Arc::new(take))
}

fn weight(morph_playback: &MorphPlayback, frame: f32, over: &MorphWeights, name: &str) -> f32 {
    let mut instance = Instance::new(Arc::clone(&MODEL));
    morph_playback.write(frame / FRAMES_PER_SECOND, over, &mut instance);
    let pmx = MODEL.extensions.get::<Pmx>().expect("a PMX");
    let at = pmx
        .morphs
        .iter()
        .position(|morph| morph.name == name)
        .expect("a morph of miku");
    instance.pose().weights(pmx.root)[at]
}

#[test]
fn pending_morphs_override_keyed_curves() {
    let morph_playback = morph_playback(Take {
        morphs: [
            key("あ", 0, 0.0, VmdCurve::LINEAR),
            key("あ", 10, 1.0, BENT),
            key("はぅ", 4, 0.5, VmdCurve::LINEAR),
        ]
        .into_iter()
        .collect(),
        ..Take::default()
    });
    let none = MorphWeights::default();
    assert!((weight(&morph_playback, 5.0, &none, "あ") - BENT.factor(0.5)).abs() < 1e-5);
    assert!((weight(&morph_playback, 5.0, &none, "はぅ") - 0.5).abs() < 1e-6);
    assert_eq!(weight(&morph_playback, 5.0, &none, "まばたき"), 0.0);
    let over: MorphWeights = [("あ".to_owned(), 0.25), ("まばたき".to_owned(), 0.75)]
        .into_iter()
        .collect();
    assert!((weight(&morph_playback, 5.0, &over, "あ") - 0.25).abs() < 1e-6);
    assert!((weight(&morph_playback, 5.0, &over, "まばたき") - 0.75).abs() < 1e-6);
    assert!((weight(&morph_playback, 5.0, &over, "はぅ") - 0.5).abs() < 1e-6);
}

#[test]
fn auto_blink_yields_to_keyed_eyes() {
    let take = |blink: bool, shut: f32| Take {
        human: Some({
            let mut human = HumanMotion::default();
            human.settings.blink = blink;
            human
        }),
        morphs: [
            key("まばたき", 0, shut, VmdCurve::LINEAR),
            key("まばたき", 900, shut, VmdCurve::LINEAR),
        ]
        .into_iter()
        .collect(),
        ..Take::default()
    };
    let none = MorphWeights::default();
    let closings = |morph_playback: &MorphPlayback| -> Vec<f32> {
        (0..900)
            .map(|frame| weight(morph_playback, frame as f32, &none, "まばたき"))
            .collect()
    };
    let blinking = closings(&morph_playback(take(true, 0.0)));
    assert!(
        blinking.iter().any(|closed| *closed > 0.99),
        "the eyes close at some point"
    );
    let open_frames = blinking.iter().filter(|closed| **closed == 0.0).count();
    assert!(open_frames > 800, "the eyes are open most of the time");
    assert!(
        closings(&morph_playback(take(false, 0.0)))
            .iter()
            .all(|closed| *closed == 0.0)
    );
    let shut = closings(&morph_playback(take(true, 0.8)));
    assert!(shut.iter().all(|closed| (*closed - 0.8).abs() < 1e-6));
}
