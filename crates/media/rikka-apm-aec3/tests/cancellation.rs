use rikka_apm_audio::FRAME;

use crate::support::fullband::{Conversation, Talk, process};
use crate::support::{db, energy};

fn window(samples: &[f32], frames: std::ops::Range<usize>) -> &[f32] {
    &samples[frames.start * FRAME..frames.end * FRAME]
}

#[test]
fn echo_alone_is_removed_after_convergence() {
    let conversation = Conversation::new(&Talk {
        frames: 800,
        delay_samples: 2_400,
        near_frames: 0..0,
    });
    let processed = process(&conversation);
    let converged = 300..800;
    let attenuation = db(energy(window(&conversation.capture, converged.clone()))
        / energy(window(&processed.output, converged)));
    assert!(
        attenuation > 23.0,
        "the echo drops by only {attenuation:.1} dB"
    );
    let erle = processed
        .metrics
        .last()
        .unwrap()
        .echo_return_loss_enhancement;
    assert!(erle > 30.0, "the reported ERLE is {erle:.1} dB");
}

#[test]
fn near_end_speech_survives_double_talk() {
    let near_frames = 500..700;
    let conversation = Conversation::new(&Talk {
        frames: 700,
        delay_samples: 2_400,
        near_frames: near_frames.clone(),
    });
    let echo_over_near = db(energy(window(&conversation.echo, near_frames.clone()))
        / energy(window(&conversation.near, near_frames.clone())));
    assert!(
        echo_over_near > 0.0,
        "the echo must be at least as loud as the near end, it is {echo_over_near:.1} dB"
    );
    let processed = process(&conversation);
    let near_level_db = db(energy(window(&processed.output, near_frames.clone()))
        / energy(window(&conversation.near, near_frames)));
    assert!(
        (-6.0..2.0).contains(&near_level_db),
        "the near end comes out {near_level_db:+.1} dB off"
    );
}
