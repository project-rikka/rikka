use rikka_apm_audio::{BANDS, FRAME, Frame, S16_SCALE, SAMPLE_RATE};

const FRAMES: usize = 20;

const ORACLE_BANDS: [[f32; 4]; BANDS] = [
    [5_692.908, 2_960.369, -2_353.657, 4_647.07],
    [9_153.19, 4_845.127, 628.355_6, -12_291.467],
    [-11_483.951, -4_011.448, 1_449.719, -5_466.373],
];
const ORACLE_MERGED: [f32; 5] = [14_814.919, -12_655.483, 4_334.762, 10_015.031, -13_617.374];

fn tone(hz: f32, frames: usize) -> Vec<f32> {
    (0..frames * FRAME)
        .map(|n| 0.5 * (std::f32::consts::TAU * hz * n as f32 / SAMPLE_RATE as f32).sin())
        .collect()
}

fn noise(frames: usize) -> Vec<f32> {
    let mut state: u32 = 12_345;
    (0..frames * FRAME)
        .map(|_| {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (((state >> 16) & 0xffff) as i32 - 32_768) as f32 / 2.0 / S16_SCALE
        })
        .collect()
}

fn close(got: f32, expected: f32) -> bool {
    (got - expected).abs() <= 1e-3 * expected.abs().max(1.0)
}

#[test]
fn tones_land_in_band() {
    for (hz, band) in [(1_000.0, 0), (12_000.0, 1), (20_000.0, 2)] {
        let mut frame = Frame::new(1);
        let mut energies = [0.0; BANDS];
        for (index, chunk) in tone(hz, FRAMES).as_chunks::<FRAME>().0.iter().enumerate() {
            frame.load(chunk, 1);
            frame.split();
            for (band, energy) in energies.iter_mut().enumerate().filter(|_| index >= 2) {
                *energy += frame.band(0, band).iter().map(|v| v * v).sum::<f32>();
            }
        }
        let total: f32 = energies.iter().sum();
        assert!(
            energies[band] / total > 0.99,
            "{hz} Hz spreads over the bands as {energies:?}"
        );
    }
}

#[test]
fn split_and_merge_match_webrtc() {
    let mut frame = Frame::new(1);
    let mut bands = [[0.0; 4]; BANDS];
    let mut merged = [0.0; 5];
    for chunk in noise(4).as_chunks::<FRAME>().0 {
        frame.load(chunk, 1);
        frame.split();
        for (band, picked) in bands.iter_mut().enumerate() {
            for (at, value) in picked.iter_mut().enumerate() {
                *value = frame.band(0, band)[at * 40];
            }
        }
        frame.merge();
        for (at, value) in merged.iter_mut().enumerate() {
            *value = frame.full(0)[at * 96];
        }
    }
    for (got, expected) in bands.iter().flatten().zip(ORACLE_BANDS.iter().flatten()) {
        assert!(
            close(*got, *expected),
            "a band holds {got}, WebRTC {expected}"
        );
    }
    for (got, expected) in merged.iter().zip(ORACLE_MERGED) {
        assert!(
            close(*got, expected),
            "the merge gives {got}, WebRTC {expected}"
        );
    }
}

#[test]
fn stereo_downmixes_and_upmixes() {
    let mut frame = Frame::new(1);
    let stereo: Vec<f32> = (0..FRAME).flat_map(|_| [0.5, -0.25]).collect();
    frame.load(&stereo, 2);
    let mut back = vec![0.0; FRAME * 2];
    frame.store(&mut back, 2);
    assert!(back.iter().all(|sample| (sample - 0.125).abs() < 1e-6));
}
