use rikka_resample::{ResampleQuality, Resampler};

const QUALITIES: [ResampleQuality; 4] = [
    ResampleQuality::Fast,
    ResampleQuality::Balanced,
    ResampleQuality::High,
    ResampleQuality::Ultra,
];

const SHARP: [ResampleQuality; 3] = [
    ResampleQuality::Balanced,
    ResampleQuality::High,
    ResampleQuality::Ultra,
];

const RATES: [(u32, u32); 5] = [
    (44100, 48000),
    (48000, 44100),
    (22050, 48000),
    (96000, 48000),
    (8000, 44100),
];

fn sine(rate: u32, frequency: f64, frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|n| (std::f64::consts::TAU * frequency * n as f64 / f64::from(rate)).sin() as f32)
        .collect()
}

fn rms(samples: &[f32]) -> f64 {
    let sum: f64 = samples.iter().map(|&s| f64::from(s) * f64::from(s)).sum();
    (sum / samples.len().max(1) as f64).sqrt()
}

fn convert(
    samples: &[f32],
    channels: u16,
    from: u32,
    to: u32,
    quality: ResampleQuality,
) -> Vec<f32> {
    let mut resampler = Resampler::with_quality(quality, from, to, channels);
    let mut output = Vec::new();
    resampler.process(samples, &mut output);
    resampler.flush(&mut output);
    output
}

#[test]
fn constant_signal_keeps_level_and_scales_length() {
    for quality in QUALITIES {
        for (from, to) in RATES {
            let frames = from as usize / 4;
            let output = convert(&vec![0.5f32; frames], 1, from, to, quality);
            let expected = (frames as u64 * u64::from(to)).div_ceil(u64::from(from)) as usize;
            assert_eq!(output.len(), expected, "{quality:?} {from} to {to}");
            let guard = 2048.min(output.len() / 4);
            for (frame, &sample) in output[guard..output.len() - guard].iter().enumerate() {
                assert!(
                    (f64::from(sample) - 0.5).abs() < 1e-4,
                    "{quality:?} {from} to {to} frame {frame}: {sample}"
                );
            }
        }
        assert!(convert(&[], 2, 44100, 48000, quality).is_empty());
    }
}

#[test]
fn tone_keeps_phase() {
    for quality in SHARP {
        for (from, to) in RATES {
            let output = convert(&sine(from, 440.0, from as usize / 2), 1, from, to, quality);
            let reference = sine(to, 440.0, output.len());
            let guard = 4096.min(output.len() / 4);
            let error = output[guard..output.len() - guard]
                .iter()
                .zip(&reference[guard..output.len() - guard])
                .map(|(&a, &b)| f64::from(a - b).abs())
                .fold(0.0, f64::max);
            assert!(error < 1e-3, "{quality:?} {from} to {to}: error {error}");
        }
    }
}

#[test]
fn downsampling_rejects_aliases() {
    for quality in SHARP {
        let passed = convert(&sine(48000, 1000.0, 48000), 1, 48000, 8000, quality);
        let level = rms(&passed[500..passed.len() - 500]);
        assert!(
            (level - std::f64::consts::FRAC_1_SQRT_2).abs() < 0.02,
            "{quality:?} lost the passband: {level}"
        );

        let blocked = convert(&sine(48000, 5000.0, 48000), 1, 48000, 8000, quality);
        let level = rms(&blocked[500..blocked.len() - 500]);
        assert!(level < 0.01, "{quality:?} let 5 kHz alias through: {level}");
    }
}

#[test]
fn chunked_processing_matches_single_call() {
    let input = sine(44100, 440.0, 30_000);
    for quality in QUALITIES {
        let offline = convert(&input, 1, 44100, 48000, quality);
        for chunk in [1, 777, 4097, 20_000] {
            let mut resampler = Resampler::with_quality(quality, 44100, 48000, 1);
            let mut streamed = Vec::new();
            for pass in input.chunks(chunk) {
                resampler.process(pass, &mut streamed);
            }
            resampler.flush(&mut streamed);
            assert_eq!(offline, streamed, "{quality:?} in {chunk}-frame chunks");

            resampler.reset();
            let mut again = Vec::new();
            resampler.process(&input, &mut again);
            resampler.flush(&mut again);
            assert_eq!(offline, again, "{quality:?} after a reset");
        }
    }
}

#[test]
fn channels_stay_independent() {
    for channels in [2u16, 6] {
        let frames = 8192;
        let mut input = Vec::with_capacity(frames * channels as usize);
        for value in sine(44100, 440.0, frames) {
            input.push(value);
            input.extend(std::iter::repeat_n(0.0, channels as usize - 1));
        }

        let output = convert(&input, channels, 44100, 48000, ResampleQuality::High);
        for channel in 1..channels as usize {
            let silent: Vec<f32> = output
                .iter()
                .skip(channel)
                .step_by(channels as usize)
                .copied()
                .collect();
            assert_eq!(rms(&silent), 0.0, "{channels} channels, channel {channel}");
        }
        let carrier: Vec<f32> = output.iter().step_by(channels as usize).copied().collect();
        assert!(rms(&carrier) > 0.6);
    }
}
