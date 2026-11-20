use std::path::PathBuf;

use rikka_audio_decode::{WavDepth, WavWriter};

fn scratch(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("rikka-audio-{}-{name}.wav", std::process::id()))
}

fn tone(frames: usize, channels: usize) -> Vec<f32> {
    (0..frames * channels)
        .map(|index| {
            let frame = (index / channels) as f32;
            let pitch = 220.0 * (1 + index % channels) as f32;
            0.8 * (std::f32::consts::TAU * pitch * frame / 48000.0).sin()
        })
        .collect()
}

#[test]
fn wav_round_trips() {
    let cases = [
        (WavDepth::Pcm16, 2u16, 4801usize, 2.5 / 32768.0),
        (WavDepth::Pcm24, 2, 4800, 1.0 / 4_000_000.0),
        (WavDepth::Pcm24, 1, 4801, 1.0 / 4_000_000.0),
        (WavDepth::Pcm16, 6, 480, 2.5 / 32768.0),
    ];
    for (depth, channels, frames, slack) in cases {
        let path = scratch(&format!("{depth:?}-{channels}-{frames}"));
        let samples = tone(frames, usize::from(channels));
        let mut writer = WavWriter::create(&path, 48000, channels, depth).unwrap();
        for block in samples.chunks(usize::from(channels) * 1000) {
            writer.write(block).unwrap();
        }
        writer.finish().unwrap();

        let sound = rikka_audio_decode::read(path.as_path()).decode().unwrap();
        let length = std::fs::metadata(&path).unwrap().len();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(length % 2, 0, "{depth:?} {channels}");
        assert_eq!(sound.sample_rate(), 48000);
        assert_eq!(sound.channels(), channels);
        assert_eq!(sound.frames(), frames as u64, "{depth:?} {channels}");
        let worst = sound
            .samples()
            .iter()
            .zip(&samples)
            .map(|(read, written)| (read - written).abs())
            .fold(0.0, f32::max);
        assert!(worst <= slack, "{depth:?} {channels}: {worst}");
    }
}
