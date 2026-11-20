use cut_waveform::{Peaks, Summary};

const RATE: u32 = 48_000;

fn signal(frames: usize, channels: usize) -> Vec<f32> {
    let mut state: u32 = 0x9e37_79b9;
    (0..frames * channels)
        .map(|index| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let noise = (state as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let swell = ((index / channels) as f32 / 9000.0).sin();
            noise * swell * 0.8
        })
        .collect()
}

fn exact_summary(
    samples: &[f32],
    channels: usize,
    channel: usize,
    frames: std::ops::Range<u64>,
) -> Summary {
    let picked: Vec<f32> = samples
        .chunks_exact(channels)
        .skip(frames.start as usize)
        .take((frames.end - frames.start) as usize)
        .map(|frame| frame[channel])
        .collect();
    let squares: f64 = picked.iter().map(|sample| f64::from(*sample).powi(2)).sum();
    Summary {
        low: picked.iter().copied().fold(f32::INFINITY, f32::min),
        high: picked.iter().copied().fold(f32::NEG_INFINITY, f32::max),
        rms: (squares / picked.len() as f64).sqrt() as f32,
    }
}

fn build_peaks(samples: &[f32], channels: u16, pieces: usize) -> Peaks {
    let mut peaks = Peaks::new(RATE, channels);
    let frame = usize::from(channels);
    let step = (samples.len() / frame).div_ceil(pieces) * frame;
    for piece in samples.chunks(step) {
        peaks.extend(piece);
    }
    peaks.finish();
    peaks
}

#[test]
fn summaries_keep_extremes_at_every_level() {
    let channels = 2;
    let frames = 700_001;
    let samples = signal(frames, channels);
    let peaks = build_peaks(&samples, channels as u16, 7);
    assert_eq!(peaks.frames(), frames as u64);
    assert!(peaks.is_complete());

    for (start, length) in [
        (0, 64),
        (0, 1),
        (1_000, 37),
        (12_345, 640),
        (99_999, 4_096),
        (250_000, 65_536),
        (0, 700_001),
        (699_990, 500),
    ] {
        let range = start..(start + length).min(frames as u64);
        for channel in 0..channels {
            let summary = peaks
                .summary(channel as u16, range.clone())
                .expect("a summary");
            let exact = exact_summary(&samples, channels, channel, range.clone());
            let tolerance = 1.0 / 16_000.0;
            assert!(
                summary.high >= exact.high - tolerance,
                "{range:?}: {summary:?} vs {exact:?}"
            );
            assert!(
                summary.low <= exact.low + tolerance,
                "{range:?}: {summary:?} vs {exact:?}"
            );
            let widened = exact_summary(
                &samples,
                channels,
                channel,
                range.start.saturating_sub(length / 4 + 64)
                    ..(range.end + length / 4 + 64).min(frames as u64),
            );
            assert!(
                summary.high <= widened.high + tolerance,
                "{range:?} reaches too far"
            );
            assert!(
                summary.low >= widened.low - tolerance,
                "{range:?} reaches too far"
            );
            assert!(
                (summary.rms - exact.rms).abs() <= exact.rms * 0.25 + 0.01,
                "{range:?}: {summary:?} vs {exact:?}"
            );
        }
    }
}

#[test]
fn mixed_summary_spans_all_channels() {
    let mut samples = Vec::new();
    for _ in 0..10_000 {
        samples.extend_from_slice(&[0.25, -0.75]);
    }
    let peaks = build_peaks(&samples, 2, 3);

    let mixed = peaks.mixed(0..10_000).expect("a summary");

    assert!((mixed.high - 0.25).abs() < 1e-3 && (mixed.low + 0.75).abs() < 1e-3);
    let left = peaks.summary(0, 0..10_000).expect("a summary");
    assert!((left.low - 0.25).abs() < 1e-3);
}

#[test]
fn partial_peaks_summarize_decoded_frames() {
    let samples = signal(10_000, 1);
    let mut peaks = Peaks::new(RATE, 1).with_expected_frames(Some(20_000));
    peaks.extend(&samples);

    assert!(!peaks.is_complete());
    assert_eq!(peaks.expected(), Some(20_000));
    assert!(peaks.summary(0, 15_000..16_000).is_none());
    let read = peaks.summary(0, 0..20_000).expect("what was read");
    let exact = exact_summary(&samples, 1, 0, 0..9_984);
    assert!((read.high - exact.high).abs() < 1e-3);
}
