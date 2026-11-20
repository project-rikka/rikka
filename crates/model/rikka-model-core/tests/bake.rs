use rikka_model_core::Transform;
use rikka_model_core::bake::{Subdivision, adaptive};
use rikka_model_core::glam::Vec3;

const FRAMES: Subdivision = Subdivision::Grid { step: 1.0 };
const TOLERANCE: f32 = 1e-3;

struct Case {
    name: &'static str,
    seeds: &'static [f64],
    motion: fn(f64) -> f64,
    expected: Option<&'static [f64]>,
}

fn smooth(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

fn held_ease(frame: f64) -> f64 {
    smooth(((frame - 40.0) / 20.0).clamp(0.0, 1.0))
}

fn stepped(frame: f64) -> f64 {
    match frame < 10.5 {
        true => 0.0,
        false => 1.0,
    }
}

fn bake(seeds: &[f64], motion: fn(f64) -> f64) -> Vec<(f64, Transform)> {
    adaptive(seeds.to_vec(), FRAMES, |frame| {
        Transform::from_translation(Vec3::X * motion(frame) as f32)
    })
}

fn between(baked: &[(f64, Transform)], frame: f64) -> f32 {
    let after = baked
        .partition_point(|(at, _)| *at < frame)
        .min(baked.len() - 1);
    let (end, last) = baked[after];
    if end <= frame || after == 0 {
        return last.translation.x;
    }
    let (start, first) = baked[after - 1];
    let factor = ((frame - start) / (end - start)) as f32;
    first.lerp(last, factor).translation.x
}

#[test]
fn adaptive_baking_is_exact_at_keys_and_sparse_where_straight() {
    let cases = [
        Case {
            name: "still",
            seeds: &[0.0, 100.0],
            motion: |_| 0.25,
            expected: Some(&[0.0, 100.0]),
        },
        Case {
            name: "linear",
            seeds: &[0.0, 100.0],
            motion: |frame| frame / 100.0,
            expected: Some(&[0.0, 100.0]),
        },
        Case {
            name: "held ease",
            seeds: &[0.0, 40.0, 60.0, 100.0],
            motion: held_ease,
            expected: None,
        },
        Case {
            name: "step",
            seeds: &[0.0, 10.5, 20.0],
            motion: stepped,
            expected: None,
        },
    ];
    for Case {
        name,
        seeds,
        motion,
        expected,
    } in cases
    {
        let baked = bake(seeds, motion);
        let times: Vec<f64> = baked.iter().map(|(at, _)| *at).collect();
        if let Some(expected) = expected {
            assert_eq!(times, expected, "{name}");
        }
        for seed in seeds {
            assert!(times.contains(seed), "{name}: key {seed} is not a sample");
        }
        for &(at, _) in &baked {
            assert!(
                at.fract() == 0.0 || seeds.contains(&at),
                "{name}: {at} is neither a key nor a frame"
            );
        }
        let last = seeds[seeds.len() - 1] as i32;
        for frame in 0..=last {
            let frame = f64::from(frame);
            let error = (between(&baked, frame) - motion(frame) as f32).abs();
            assert!(
                error <= TOLERANCE,
                "{name}: frame {frame} is off by {error}"
            );
        }
    }
}

#[test]
fn adaptive_baking_refines_only_where_motion_bends() {
    let baked = bake(&[0.0, 40.0, 60.0, 100.0], held_ease);
    let bent = baked
        .iter()
        .filter(|(at, _)| *at > 40.0 && *at < 60.0)
        .count();
    let held = baked
        .iter()
        .filter(|(at, _)| (*at > 0.0 && *at < 40.0) || (*at > 60.0 && *at < 100.0))
        .count();
    assert_eq!(held, 0);
    assert!(bent > 0);
}

#[test]
fn halving_stops_after_its_rounds() {
    let baked = adaptive(vec![0.0, 1.0], Subdivision::Halves { rounds: 4 }, |time| {
        Transform::from_translation(Vec3::X * (time * 40.0).sin() as f32)
    });
    assert!(baked.len() <= 17, "{} samples", baked.len());
    assert!(baked.len() > 2);
}
