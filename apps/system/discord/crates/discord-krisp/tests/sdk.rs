use discord_krisp::{FRAME, Module, SAMPLE_RATE, Sdk};

const FRAMES: usize = 300;

fn sdk() -> Sdk {
    let module = Module::cached(std::path::Path::new(env!("RIKKA_DISCORD_KRISP"))).unwrap();
    let working = std::env::temp_dir().join("krisp");
    Sdk::load(&module, &working).unwrap()
}

fn noise(state: &mut u32) -> f32 {
    *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    (*state >> 8) as f32 / (1 << 24) as f32 * 2.0 - 1.0
}

fn voice(index: usize) -> f32 {
    let time = index as f32 / SAMPLE_RATE as f32;
    let syllables = 0.5 + 0.5 * (std::f32::consts::TAU * 3.0 * time).sin();
    let pitch = std::f32::consts::TAU * 180.0 * time;
    syllables * (0.3 * pitch.sin() + 0.15 * (2.0 * pitch).sin() + 0.08 * (3.0 * pitch).sin())
}

fn power_db(samples: &[f32]) -> f32 {
    let energy = samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32;
    10.0 * energy.max(1e-12).log10()
}

#[test]
fn krisp_nc_removes_noise_keeps_speech() {
    let sdk = sdk();
    let mut cleaner = sdk.noise_cleaner().unwrap();
    let mut state = 7;
    let (mut noise_in, mut noise_out, mut voice_in, mut voice_out) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for index in 0..FRAMES {
        let speaking = index >= FRAMES / 2;
        let mut frame = [0.0; FRAME];
        for (offset, sample) in frame.iter_mut().enumerate() {
            let voiced = if speaking {
                voice(index * FRAME + offset)
            } else {
                0.0
            };
            *sample = 0.1 * noise(&mut state) + voiced;
        }
        let before = frame;
        cleaner.process(&mut frame).unwrap();
        match (speaking, index % (FRAMES / 2) >= 50) {
            (false, true) => {
                noise_in.extend_from_slice(&before);
                noise_out.extend_from_slice(&frame);
            }
            (true, true) => {
                voice_in.extend_from_slice(&before);
                voice_out.extend_from_slice(&frame);
            }
            _ => {}
        }
    }

    let removed = power_db(&noise_in) - power_db(&noise_out);
    let voice_loss_db = power_db(&voice_in) - power_db(&voice_out);
    assert!(
        removed > 30.0,
        "Krisp removes only {removed:.1} dB of noise"
    );
    assert!(
        voice_loss_db < 3.0,
        "Krisp takes {voice_loss_db:.1} dB off the voice"
    );
}

#[test]
fn krisp_vad_separates_speech_from_noise() {
    let sdk = sdk();
    let mut detector = sdk.voice_detector().unwrap();
    let mut state = 11;
    let mut probability_for = |voiced: bool, from: usize| {
        let mut probabilities = Vec::new();
        for index in from..from + 100 {
            let mut frame = [0.0; FRAME];
            for (offset, sample) in frame.iter_mut().enumerate() {
                let speech = if voiced {
                    voice(index * FRAME + offset)
                } else {
                    0.0
                };
                *sample = 0.05 * noise(&mut state) + speech;
            }
            probabilities.push(detector.probability(&frame).unwrap());
        }
        probabilities[50..].iter().sum::<f32>() / 50.0
    };

    let quiet = probability_for(false, 0);
    let speaking = probability_for(true, 100);

    assert!(quiet < 0.3, "noise alone looks like a voice ({quiet:.2})");
    assert!(speaking > 0.7, "a voice is missed ({speaking:.2})");
}

#[test]
fn sdk_sessions_run_concurrently() {
    let first = sdk();
    let second = sdk();
    let mut cleaners = [
        first.noise_cleaner().unwrap(),
        second.noise_cleaner().unwrap(),
    ];
    let mut frame = [0.01; FRAME];
    for cleaner in &mut cleaners {
        cleaner.process(&mut frame).unwrap();
    }
}
