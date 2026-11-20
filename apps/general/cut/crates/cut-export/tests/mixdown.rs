use std::sync::Arc;

use cut_export::{CHANNELS, Mixdown};
use cut_project::{Edit, Fade, FadeShape, KeyChange, MixField, Project, Property, Value};

use crate::support::{SAMPLE_RATE, click, mixed, placed, sound, tone, treated};

const PER_FRAME: usize = (SAMPLE_RATE / 30) as usize;

fn mixdown(project: Project, first: i64, frames: usize) -> Vec<f32> {
    let mut mixdown = Mixdown::new(Arc::new(project), first, SAMPLE_RATE);
    let mut out = vec![0.0; frames * usize::from(CHANNELS)];
    for block in out.chunks_mut(997 * usize::from(CHANNELS)) {
        mixdown.fill(block);
    }
    assert!(mixdown.undecodable().is_empty());
    out
}

fn loudest(out: &[f32], channel: usize) -> (usize, f32) {
    out.iter()
        .skip(channel)
        .step_by(usize::from(CHANNELS))
        .enumerate()
        .fold((0, 0.0), |best, (at, value)| match value.abs() > best.1 {
            true => (at, value.abs()),
            false => best,
        })
}

#[test]
fn clip_sounds_at_timeline_position() {
    let folder = tempfile::tempdir().expect("a folder");
    let media = sound(&folder.path().join("click.wav"), &click(48_000, 4800), 1);
    let cases = [
        (0, 0, 1.0, 4800),
        (15, 0, 1.0, 15 * PER_FRAME + 4800),
        (15, 10, 1.0, 5 * PER_FRAME + 4800),
        (0, 0, 2.0, 2400),
        (6, 0, 0.5, 6 * PER_FRAME + 9600),
    ];
    for (at, first, speed, expected) in cases {
        let (project, clip) = placed(&Project::new(), media.clone(), at, 0);
        let project = treated(&project, clip, |treatment| treatment.speed = speed);
        let out = mixdown(project, first, 60_000);
        let (left, right) = (loudest(&out, 0), loudest(&out, 1));
        assert!(
            left.0.abs_diff(expected) <= 1,
            "{at} {first} {speed}: {left:?}"
        );
        assert_eq!(left.0, right.0);
        assert!((left.1 - right.1).abs() < 1e-4);
        if speed == 1.0 {
            assert!(
                (left.1 - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-3,
                "{left:?}"
            );
        }
    }
}

#[test]
fn volume_pan_and_mute_shape_mix() {
    let folder = tempfile::tempdir().expect("a folder");
    let media = sound(&folder.path().join("tone.wav"), &tone(48_000, 0.5), 1);
    let centre = 0.5 * std::f32::consts::FRAC_1_SQRT_2;
    let cases = [
        (mixed(1.0, 0.0, false), [centre, centre]),
        (mixed(0.5, 1.0, false), [0.0, 0.25]),
        (mixed(2.0, -1.0, false), [1.0, 0.0]),
        (mixed(1.0, 0.0, true), [0.0, 0.0]),
    ];
    for (mix, expected) in cases {
        let (project, clip) = placed(&Project::new(), media.clone(), 0, 0);
        let project = treated(&project, clip, |treatment| treatment.mix = mix);
        let out = mixdown(project, 0, 24_000);
        for (channel, expected_peak) in expected.into_iter().enumerate() {
            let (_, peak) = loudest(&out, channel);
            assert!(
                (peak - expected_peak).abs() < 0.01,
                "{mix:?} channel {channel}: {peak}"
            );
        }
    }
}

#[test]
fn volume_keys_and_fades_shape_mix() {
    let folder = tempfile::tempdir().expect("a folder");
    let media = sound(&folder.path().join("tone.wav"), &tone(48_000, 0.5), 1);
    let centre = 0.5 * std::f32::consts::FRAC_1_SQRT_2;
    let (project, clip) = placed(&Project::new(), media, 0, 0);
    let faded = treated(&project, clip, |treatment| {
        treatment.mix.fade_in = Fade {
            length: 15,
            shape: FadeShape::Linear,
        };
    });
    let volume = Property::Mix(MixField::Volume);
    let keyed = [
        KeyChange::Add { at: 0 },
        KeyChange::Add { at: 20 },
        KeyChange::Value {
            at: 20,
            value: Value::Scalar(0.0),
        },
    ]
    .into_iter()
    .fold(project, |project, change| {
        project
            .apply(&Edit::Key {
                clip,
                property: volume.clone(),
                change,
            })
            .expect("keying")
            .0
    });
    let window = |out: &[f32], frame: usize| {
        let from = frame * PER_FRAME * usize::from(CHANNELS);
        let to = from + PER_FRAME * usize::from(CHANNELS);
        loudest(&out[from..to], 0).1
    };

    let out = mixdown(faded, 0, 30 * PER_FRAME);
    assert!(window(&out, 1) <= centre * 2.0 / 15.0 + 0.01);
    assert!((window(&out, 7) - centre * 8.0 / 15.0).abs() < 0.02);
    assert!((window(&out, 20) - centre).abs() < 0.01);

    let out = mixdown(keyed, 0, 30 * PER_FRAME);
    assert!((window(&out, 0) - centre).abs() < 0.01);
    assert!((window(&out, 10) - centre * 0.5).abs() < 0.02);
    assert!(window(&out, 25) < 0.001);
}
