use animate_fluid::{
    Emitter, EmitterId, EmitterKey, EmitterLevels, EmitterSetting, EmitterValue, Pour, Shape,
};
use rikka_mmd::VmdCurve;
use rikka_model_core::glam::Vec3;

use crate::support::key;

fn blob() -> Emitter {
    Emitter::new(EmitterId(1), "palm", Shape::blob())
}

fn hose() -> Emitter {
    let mut emitter = Emitter::new(EmitterId(3), "hose", Shape::Stream);
    for (frame, rate) in [(10, 0.1), (40, 0.3), (90, 0.2)] {
        emitter
            .tracks_mut()
            .insert(key(EmitterSetting::Rate, frame, EmitterValue::Number(rate)));
    }
    emitter
}

fn partitioned(emitter: &Emitter, seed: u64, end: f32) -> Vec<u32> {
    let mut state = seed;
    let mut next = || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 40) as f32 / (1u64 << 24) as f32
    };
    let mut landings = Vec::new();
    let mut after = f32::NEG_INFINITY;
    let mut through = 0.0;
    while after < end {
        if emitter.lands_between(after, through) {
            landings.push(emitter.pour.start());
        }
        after = through;
        through += match next() {
            step if step < 0.2 => 1.0,
            step if step < 0.4 => step * 0.5,
            step => step * 9.0,
        };
    }
    landings
}

#[test]
fn unkeyed_settings_use_rest_levels() {
    let mut emitter = Emitter::new(EmitterId(0), "hose", Shape::Stream);
    assert_eq!(emitter.sample(12.0).levels, EmitterLevels::default());
    assert_eq!(emitter.sample(12.0).launch.offset, Emitter::LAUNCH);
    emitter
        .tracks_mut()
        .insert(key(EmitterSetting::Rate, 10, EmitterValue::Number(0.2)));
    emitter.tracks_mut().insert(EmitterKey {
        curve: VmdCurve {
            first: (127, 0),
            second: (127, 0),
        },
        ..key(EmitterSetting::Rate, 20, EmitterValue::Number(0.4))
    });
    emitter.tracks_mut().insert(key(
        EmitterSetting::Launch,
        0,
        EmitterValue::Three([0.0, 1.0, 0.0]),
    ));
    emitter.tracks_mut().insert(key(
        EmitterSetting::Launch,
        10,
        EmitterValue::Three([1.0, 1.0, 0.0]),
    ));
    emitter.pour = Pour::new(0, 30);
    let state = emitter.sample(5.0);
    assert_eq!(state.levels.rate, 0.2);
    assert_eq!(state.launch.offset, Vec3::new(0.5, 1.0, 0.0));
    let eased = emitter.sample(15.0).levels.rate;
    assert!(eased > 0.2 && eased < 0.3, "{eased}");
    assert_eq!(emitter.sample(99.0).levels.rate, 0.4);
    assert_eq!(
        emitter.sample(99.0).levels.width,
        EmitterLevels::default().width
    );
    assert_eq!(emitter.last_frame(), 30);
}

#[test]
fn stream_pours_for_its_span() {
    let mut hose = Emitter::new(EmitterId(4), "hose", Shape::Stream);
    hose.pour = Pour::from_start(30).lasting_seconds(1.5);
    assert_eq!(hose.pour.length(), 45);
    for (frame, on) in [(29.9, false), (30.0, true), (74.9, true), (75.0, false)] {
        assert_eq!(hose.sample(frame).on, on, "{frame}");
    }
    assert_eq!(hose.last_frame(), 75);
    let litres = hose.litres();
    assert!(
        (litres - EmitterLevels::default().rate * 1.5).abs() < 1e-4,
        "{litres}"
    );
    assert_eq!(hose.pour.starting_at(60), Pour::new(60, 15));
    assert_eq!(hose.pour.starting_at(200), Pour::new(74, 1));
    assert_eq!(hose.pour.ending_at(10), Pour::new(30, 1));
}

#[test]
fn blob_lands_once_at_start() {
    for start in [0, 12, 81] {
        let mut emitter = blob();
        emitter.pour = Pour::from_start(start);
        for seed in 0..64 {
            assert_eq!(partitioned(&emitter, seed, 200.0), [start], "{seed}");
        }
        assert_eq!(emitter.last_frame(), start);
    }
    let mut emitter = blob();
    emitter.pour = Pour::from_start(40);
    assert!(emitter.lands_between(12.0, 40.0));
    assert!(!emitter.lands_between(40.0, 80.9));
    assert!(!emitter.sample(39.0).on && emitter.sample(41.0).on);
    let stream = Emitter::new(EmitterId(2), "hose", Shape::Stream);
    assert!(!stream.lands_between(f32::NEG_INFINITY, 1e6));
}

#[test]
fn moved_key_changes_from_previous_key() {
    let before = hose();
    assert_eq!(before.changed_from(&before), None);
    let mut after = before.clone();
    after
        .tracks_mut()
        .insert(key(EmitterSetting::Rate, 90, EmitterValue::Number(0.5)));
    assert_eq!(after.changed_from(&before), Some(40));
    let mut added = before.clone();
    added
        .tracks_mut()
        .insert(key(EmitterSetting::Width, 60, EmitterValue::Number(0.02)));
    assert_eq!(added.changed_from(&before), Some(0));
    let mut later = before.clone();
    later
        .tracks_mut()
        .insert(key(EmitterSetting::Rate, 120, EmitterValue::Number(0.0)));
    assert_eq!(later.changed_from(&before), Some(90));
    assert_eq!(before.changed_from(&later), Some(90));
    let mut renamed = before.clone();
    renamed.name = "tap".into();
    assert_eq!(renamed.changed_from(&before), None, "a name pours nothing");
}

#[test]
fn retimed_pour_changes_from_earlier_edge() {
    let before = hose();
    let mut longer = before.clone();
    longer.pour = before.pour.lasting(90);
    assert_eq!(longer.changed_from(&before), Some(60));
    assert_eq!(before.changed_from(&longer), Some(60));
    let mut later = before.clone();
    later.pour = before.pour.moved_to(25);
    assert_eq!(later.changed_from(&before), Some(0));
    let mut trimmed = before.clone();
    trimmed.pour = Pour::new(30, 30);
    assert_eq!(trimmed.changed_from(&before), Some(0));
    let mut thrown = blob();
    thrown.pour = Pour::from_start(20);
    let mut lasting = thrown.clone();
    lasting.pour = thrown.pour.lasting(5);
    assert_eq!(lasting.changed_from(&thrown), None, "a blob lands at once");
}

#[test]
fn keyed_rate_litres_integrate_the_rate_curve() {
    let mut hose = hose();
    hose.tracks_mut().insert(EmitterKey {
        curve: VmdCurve {
            first: (100, 0),
            second: (27, 127),
        },
        ..key(EmitterSetting::Rate, 60, EmitterValue::Number(0.5))
    });
    for pour in [Pour::new(0, 120), Pour::new(25, 50), Pour::new(95, 30)] {
        hose.pour = pour;
        let steps = 400;
        let start = pour.start() as f32;
        let width = pour.length() as f32 / steps as f32;
        let fine: f32 = (0..steps)
            .map(|at| {
                let frame = start + (at as f32 + 0.5) * width;
                hose.value_at(EmitterSetting::Rate, frame)
                    .number()
                    .unwrap_or_default()
                    * width
            })
            .sum::<f32>()
            / 30.0;
        let litres = hose.litres();
        assert!(
            (litres - fine).abs() < 1e-3 * fine.max(1.0),
            "{pour:?}: {litres} vs {fine}"
        );
    }
}
