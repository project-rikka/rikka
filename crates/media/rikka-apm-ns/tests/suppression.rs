use rikka_apm_audio::{BAND, BANDS, Frame};
use rikka_apm_ns::{Level, NoiseSuppressor};

use crate::support::{BURSTS, Scene, burst};

const NOMINAL: [(Level, f64); 4] = [
    (Level::Low, -6.0),
    (Level::Moderate, -12.0),
    (Level::High, -18.0),
    (Level::VeryHigh, -20.9),
];

fn energy(samples: &[f32]) -> f64 {
    samples
        .iter()
        .map(|&sample| f64::from(sample) * f64::from(sample))
        .sum()
}

fn decibels(ratio: f64) -> f64 {
    10.0 * ratio.log10()
}

#[test]
fn attenuates_stationary_noise() {
    for (level, nominal) in NOMINAL {
        let mut suppressor = NoiseSuppressor::new(level, 1);
        let mut frame = Frame::new(1);
        let mut scene = Scene::noise_only([1000.0; BANDS]);
        let mut input = [0.0f64; BANDS];
        let mut output = [0.0f64; BANDS];
        for index in 0..400 {
            scene.fill(&mut frame);
            let measured = index >= 300;
            if measured {
                for (band, total) in input.iter_mut().enumerate() {
                    *total += energy(frame.band(0, band));
                }
            }
            suppressor.analyze(&frame);
            suppressor.process(&mut frame);
            if measured {
                for (band, total) in output.iter_mut().enumerate() {
                    *total += energy(frame.band(0, band));
                }
            }
        }
        for band in 0..BANDS {
            let attenuation = decibels(output[band] / input[band]);
            assert!(
                (attenuation - nominal).abs() <= 2.0,
                "{level:?} band {band}: {attenuation:.2} dB, nominal {nominal} dB"
            );
        }
    }
}

#[test]
fn preserves_voiced_burst_over_noise() {
    let (start, length) = BURSTS[0];
    for (level, _) in NOMINAL {
        let mut suppressor = NoiseSuppressor::new(level, 1);
        let mut frame = Frame::new(1);
        let mut scene = Scene::new(1);
        let mut clean = 0.0f64;
        let mut output = 0.0f64;
        for index in 0..start + length + 1 {
            scene.fill(&mut frame);
            if (start..start + length).contains(&index) {
                let voice: Vec<f32> = (0..BAND)
                    .map(|sample| burst(index, sample, 0, 0) as f32)
                    .collect();
                clean += energy(&voice);
            }
            suppressor.analyze(&frame);
            suppressor.process(&mut frame);
            if (start + 1..=start + length).contains(&index) {
                output += energy(frame.band(0, 0));
            }
        }
        let change = decibels(output / clean);
        assert!(
            change.abs() <= 1.0,
            "{level:?}: burst changed by {change:.2} dB"
        );
    }
}
