use rikka_apm_agc2::{Config, GainController};

use crate::golden;
use crate::support::{Synth, adaptive, run};

const TOLERANCE: f64 = 1e-6;

fn deviation(actual: f64, expected: f64) -> f64 {
    (actual - expected).abs() / expected.abs().max(1.0)
}

#[test]
fn vad_probabilities_match_oracle() {
    let mut controller = GainController::new(&adaptive(50.0), 1);
    let mut worst = 0.0f64;
    run(
        &mut controller,
        &mut [Synth::new(7, 4.0, 3.0, 240)],
        golden::VAD.len(),
        |_| None,
        |index, _, controller| {
            let probability = controller.speech_probability().unwrap();
            worst = worst.max(deviation(
                f64::from(probability),
                f64::from(golden::VAD[index]),
            ));
        },
    );
    assert!(worst < TOLERANCE, "largest probability deviation {worst}");
}

struct Case {
    name: &'static str,
    config: Config,
    synths: Vec<Synth>,
    external_probability: bool,
    expected: &'static [[f64; 3]],
}

#[test]
fn gain_controller_matches_oracle() {
    let given = |index: usize| Some(if index % 50 < 30 { 0.98 } else { 0.1 });
    let cases = [
        Case {
            name: "mono",
            config: adaptive(50.0),
            synths: vec![Synth::new(11, 2.0, 2.0, 230)],
            external_probability: false,
            expected: &golden::MONO,
        },
        Case {
            name: "stereo with fixed gain",
            config: Config {
                fixed_gain_db: 6.0,
                ..adaptive(50.0)
            },
            synths: vec![
                Synth::new(13, 30.0, 4.0, 250),
                Synth::new(17, 20.0, 4.0, 270),
            ],
            external_probability: false,
            expected: &golden::STEREO,
        },
        Case {
            name: "given probability into the limiter",
            config: adaptive(30.0),
            synths: vec![Synth::new(19, 120.0, 8.0, 260)],
            external_probability: true,
            expected: &golden::GIVEN,
        },
    ];
    for mut case in cases {
        let channels = case.synths.len();
        let mut controller = GainController::new(&case.config, channels);
        let mut worst = 0.0f64;
        run(
            &mut controller,
            &mut case.synths,
            case.expected.len(),
            |index| {
                if case.external_probability {
                    given(index)
                } else {
                    None
                }
            },
            |index, frame, _| {
                let [energy, first, last] = case.expected[index];
                let actual: f64 = frame
                    .full(0)
                    .iter()
                    .map(|&x| f64::from(x) * f64::from(x))
                    .sum();
                worst = worst
                    .max(deviation(actual, energy))
                    .max(deviation(f64::from(frame.full(0)[100]), first))
                    .max(deviation(f64::from(frame.full(channels - 1)[479]), last));
            },
        );
        assert!(
            worst < TOLERANCE,
            "{}: largest relative deviation {worst}",
            case.name
        );
    }
}
