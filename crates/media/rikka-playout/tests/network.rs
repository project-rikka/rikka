use std::time::Duration;

use rikka_playout::SpeechType;

use crate::support::{Network, count_glitches, silent_tail, simulate, speech_span};

const PACKETS: u32 = 500;

fn lcg(seed: u32) -> impl FnMut(u32) -> u32 {
    let mut state = seed;
    move |_| {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        state >> 8
    }
}

#[test]
fn steady_stream_plays_without_gaps() {
    let run = simulate(Network::steady(), PACKETS, silent_tail(PACKETS), 11.0);

    let span = speech_span(&run.speech);
    assert!(
        span.iter().all(|kind| *kind == SpeechType::Speech),
        "a steady stream was concealed"
    );
    assert_eq!(count_glitches(&run.output), 0);
    let steady = run.targets[run.targets.len() / 2];
    assert!(
        steady <= Duration::from_millis(40),
        "a clean network buffers {steady:?}"
    );
}

#[test]
fn jitter_absorbed_then_buffer_shrinks() {
    let mut random = lcg(3);
    let network = Network {
        delay: Box::new(move |index| {
            let spread = if index < 250 { random(index) % 60 } else { 0 };
            Duration::from_millis(20 + u64::from(spread))
        }),
        lost: Box::new(|_| false),
        sender_speed: 1.0,
    };

    let run = simulate(network, PACKETS, None, 11.0);

    let span = speech_span(&run.speech);
    let concealed = span
        .iter()
        .filter(|kind| **kind == SpeechType::Concealed)
        .count();
    assert!(
        concealed * 100 <= span.len() * 3,
        "{concealed} of {} chunks were concealed under jitter",
        span.len()
    );
    let jittery = run.targets[400];
    let calm = run.targets[run.targets.len() - 120];
    assert!(
        jittery >= Duration::from_millis(50),
        "jitter of 60 ms buffers only {jittery:?}"
    );
    assert!(
        calm < jittery,
        "the buffer stays at {calm:?} after the jitter ends"
    );
    assert!(run.buffered[run.buffered.len() - 120] < Duration::from_millis(120));
}

#[test]
fn lost_packet_recovered_from_fec() {
    let network = Network {
        delay: Box::new(|_| Duration::from_millis(40)),
        lost: Box::new(|index| index == 100 || index == 300),
        sender_speed: 1.0,
    };

    let run = simulate(network, PACKETS, silent_tail(PACKETS), 11.0);

    assert_eq!(run.counts.recovered.get(), 2);
    assert_eq!(run.counts.concealed.get(), 0);
    assert_eq!(count_glitches(&run.output), 0);
}

#[test]
fn talkspurt_end_is_not_concealed() {
    let run = simulate(Network::steady(), 200, silent_tail(200), 6.0);

    assert_eq!(run.counts.concealed.get(), 0);
    assert!(
        run.speech
            .iter()
            .rev()
            .take(100)
            .all(|kind| *kind == SpeechType::Silence)
    );
}

#[test]
fn cut_stream_conceals_then_goes_silent() {
    let run = simulate(Network::steady(), 200, None, 6.0);

    assert!((1..=12).contains(&run.counts.concealed.get()));
    assert!(
        run.speech
            .iter()
            .rev()
            .take(100)
            .all(|kind| *kind == SpeechType::Silence)
    );
}

#[test]
fn fast_sender_clock_keeps_buffer_bounded() {
    let network = Network {
        delay: Box::new(|_| Duration::from_millis(40)),
        lost: Box::new(|_| false),
        sender_speed: 1.01,
    };

    let run = simulate(network, 1000, None, 20.0);

    let span = speech_span(&run.speech);
    assert!(span.iter().all(|kind| *kind == SpeechType::Speech));
    let peak = run.buffered[..run.buffered.len() - 200]
        .iter()
        .max()
        .copied()
        .unwrap();
    assert!(
        peak < Duration::from_millis(150),
        "a drifting clock piles up {peak:?}"
    );
}

#[test]
fn burst_after_stall_returns_to_low_latency() {
    let network = Network {
        delay: Box::new(|index| {
            if (100..110).contains(&index) {
                Duration::from_millis(40 + u64::from(110 - index) * 20)
            } else {
                Duration::from_millis(40)
            }
        }),
        lost: Box::new(|_| false),
        sender_speed: 1.0,
    };

    let run = simulate(network, PACKETS, silent_tail(PACKETS), 11.0);

    assert_eq!(run.counts.recovered.get(), 0);
    assert_eq!(
        run.counts.decoded.get(),
        PACKETS,
        "the stalled packets were dropped"
    );
    assert!(
        run.counts.concealed.get() <= 12,
        "the stall is concealed for too long"
    );
    assert_eq!(count_glitches(&run.output), 0);
    let late = run.buffered[run.buffered.len() - 200];
    assert!(
        late <= Duration::from_millis(100),
        "the burst still sits in the buffer: {late:?}"
    );
}
