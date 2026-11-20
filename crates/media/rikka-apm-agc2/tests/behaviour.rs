use rikka_apm_agc2::{Config, GainController};
use rikka_apm_audio::FRAME;

use crate::support::{Synth, adaptive, rms_dbfs, run};

const SECOND: usize = 100;

#[test]
fn quiet_speech_is_raised_to_target() {
    let mut controller = GainController::new(&adaptive(50.0), 1);
    let mut input = Synth::new(23, 1.0, 0.5, 240);
    let mut reference = input.clone();
    let mut voiced = Vec::new();
    run(
        &mut controller,
        std::slice::from_mut(&mut input),
        10 * SECOND,
        |_| None,
        |index, frame, _| {
            let mut original = [0.0f32; FRAME];
            reference.fill(&mut original);
            if index >= 8 * SECOND && rms_dbfs(&original) > -70.0 {
                voiced.push((rms_dbfs(&original), rms_dbfs(frame.full(0))));
            }
        },
    );
    let mean =
        |pick: fn(&(f32, f32)) -> f32| voiced.iter().map(pick).sum::<f32>() / voiced.len() as f32;
    let input_dbfs = mean(|levels| levels.0);
    let output_dbfs = mean(|levels| levels.1);
    assert!(
        input_dbfs < -50.0,
        "the input speech is not quiet: {input_dbfs} dBFS"
    );
    assert!(
        (-30.0..-15.0).contains(&output_dbfs),
        "output speech at {output_dbfs} dBFS from {input_dbfs} dBFS"
    );
    assert!(
        controller.gain_db() > 25.0,
        "gain only {} dB",
        controller.gain_db()
    );
}

#[test]
fn limiter_keeps_speech_below_full_scale() {
    let config = Config {
        fixed_gain_db: 20.0,
        ..Config::default()
    };
    let mut controller = GainController::new(&config, 2);
    let mut peak = 0.0f32;
    let mut at_full_scale = 0usize;
    let mut deepest_limiting_db = 0.0f32;
    let frames = 3 * SECOND;
    run(
        &mut controller,
        &mut [
            Synth::new(29, 60.0, 4.0, 250),
            Synth::new(31, 45.0, 4.0, 270),
        ],
        frames,
        |_| None,
        |_, frame, controller| {
            for channel in 0..2 {
                for &x in frame.full(channel) {
                    peak = peak.max(x.abs());
                    at_full_scale += usize::from(x.abs() >= 32767.0);
                }
            }
            deepest_limiting_db = deepest_limiting_db.min(controller.limiter_gain_db());
        },
    );
    assert!(peak <= 32768.0, "peak {peak}");
    assert!(
        deepest_limiting_db < -12.0,
        "the limiter only reduced by {deepest_limiting_db} dB"
    );
    let samples = frames * FRAME * 2;
    assert!(
        at_full_scale * 1000 < samples,
        "{at_full_scale} of {samples} samples reached full scale"
    );
}

#[test]
fn noise_stays_below_noise_limit() {
    for probability in [None, Some(1.0)] {
        let config = adaptive(50.0);
        let mut controller = GainController::new(&config, 1);
        let mut loudest_dbfs = f32::MIN;
        run(
            &mut controller,
            &mut [Synth::new(37, 0.0, 56.8, 240)],
            8 * SECOND,
            |_| probability,
            |index, frame, _| {
                if index >= 5 * SECOND {
                    loudest_dbfs = loudest_dbfs.max(rms_dbfs(frame.full(0)));
                }
            },
        );
        let limit_dbfs = config.adaptive.max_output_noise_level_dbfs;
        let noise_dbfs = controller.noise_level_dbfs().unwrap();
        assert!(
            controller.gain_db() <= limit_dbfs - noise_dbfs + 0.01,
            "{probability:?}: gain {} dB over noise at {noise_dbfs} dBFS",
            controller.gain_db()
        );
        assert!(
            loudest_dbfs < limit_dbfs + 1.5,
            "{probability:?}: output noise at {loudest_dbfs} dBFS"
        );
    }
}
