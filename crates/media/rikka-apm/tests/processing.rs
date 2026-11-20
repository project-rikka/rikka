use rikka_apm::{Config, FRAME, NoiseLevel, Processor, SAMPLE_RATE};

const FRAMES: usize = 600;
const ECHO_DELAY: usize = 1_920;

fn speech(index: usize) -> f32 {
    let time = index as f32 / SAMPLE_RATE as f32;
    let syllables = (0.5 + 0.5 * (std::f32::consts::TAU * 2.3 * time).sin()).powi(2);
    let pitch =
        std::f32::consts::TAU * (140.0 + 30.0 * (std::f32::consts::TAU * 0.7 * time).sin()) * time;
    syllables
        * (0.25 * pitch.sin()
            + 0.12 * (2.0 * pitch).sin()
            + 0.06 * (3.0 * pitch).sin()
            + 0.03 * (5.0 * pitch).sin())
}

fn power_db(samples: &[f32]) -> f32 {
    let energy =
        samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len().max(1) as f32;
    10.0 * energy.max(1e-12).log10()
}

#[test]
fn removes_echo() {
    let config = Config {
        high_pass: true,
        echo_cancellation: true,
        noise_suppression: None,
        gain_control: false,
    };
    let (mut processor, reference) = Processor::new(&config);
    let far: Vec<f32> = (0..FRAMES * FRAME + ECHO_DELAY).map(speech).collect();
    let (mut before, mut after) = (Vec::new(), Vec::new());
    for index in 0..FRAMES {
        let played = &far[index * FRAME..(index + 1) * FRAME];
        let stereo: Vec<f32> = played
            .iter()
            .flat_map(|sample| [*sample, *sample])
            .collect();
        reference.render(&stereo, 2);
        let mut captured = [0.0; FRAME];
        for (offset, sample) in captured.iter_mut().enumerate() {
            let at = index * FRAME + offset;
            *sample = if at >= ECHO_DELAY {
                0.3 * far[at - ECHO_DELAY]
            } else {
                0.0
            };
        }
        let input = captured;
        processor.clean(&mut captured);
        if index >= FRAMES / 2 {
            before.extend_from_slice(&input);
            after.extend_from_slice(&captured);
        }
    }

    let removed = power_db(&before) - power_db(&after);
    assert!(removed > 20.0, "the echo is only {removed:.1} dB quieter");
}

#[test]
fn disabled_processing_is_passthrough() {
    let config = Config {
        high_pass: false,
        echo_cancellation: false,
        noise_suppression: None,
        gain_control: false,
    };
    let (mut processor, _) = Processor::new(&config);
    let mut frame = [0.0; FRAME];
    for (index, sample) in frame.iter_mut().enumerate() {
        *sample = speech(index);
    }
    let original = frame;

    processor.clean(&mut frame);
    processor.level(&mut frame, None);

    for (got, expected) in frame.iter().zip(original) {
        assert!((got - expected).abs() < 1e-4);
    }
}

#[test]
fn standard_suppression_quietens_steady_noise() {
    let config = Config {
        high_pass: true,
        echo_cancellation: false,
        noise_suppression: Some(NoiseLevel::High),
        gain_control: false,
    };
    let (mut processor, _) = Processor::new(&config);
    let mut state: u32 = 9;
    let (mut before, mut after) = (Vec::new(), Vec::new());
    for index in 0..400 {
        let mut frame = [0.0; FRAME];
        for sample in &mut frame {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *sample = 0.05 * ((state >> 8) as f32 / (1 << 24) as f32 * 2.0 - 1.0);
        }
        let input = frame;
        processor.clean(&mut frame);
        if index >= 200 {
            before.extend_from_slice(&input);
            after.extend_from_slice(&frame);
        }
    }

    let removed = power_db(&before) - power_db(&after);
    assert!(removed > 12.0, "steady noise drops only {removed:.1} dB");
}

#[test]
fn gain_control_raises_quiet_talker() {
    let config = Config {
        high_pass: true,
        echo_cancellation: false,
        noise_suppression: None,
        gain_control: true,
    };
    let (mut processor, _) = Processor::new(&config);
    let (mut before, mut after) = (Vec::new(), Vec::new());
    for index in 0..1_000 {
        let mut frame = [0.0; FRAME];
        for (offset, sample) in frame.iter_mut().enumerate() {
            *sample = 0.05 * speech(index * FRAME + offset);
        }
        let input = frame;
        processor.clean(&mut frame);
        processor.level(&mut frame, None);
        if index >= 800 {
            before.extend_from_slice(&input);
            after.extend_from_slice(&frame);
        }
    }

    let raised = power_db(&after) - power_db(&before);
    assert!(
        raised > 10.0,
        "a quiet talker is raised by only {raised:.1} dB"
    );
    assert!(after.iter().all(|sample| sample.abs() <= 1.0));
}
