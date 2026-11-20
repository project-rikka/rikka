use std::time::Duration;

use rikka_playout::SpeechType;

use crate::support::{Device, Network, count_glitches, silent_tail, simulate_device, speech_span};

const PACKETS: u32 = 500;
const SECONDS: f64 = 11.0;
const LEARNING: Duration = Duration::from_secs(1);

fn jittery(seed: u32, spread_ms: u32) -> Network {
    let mut state = seed;
    Network {
        delay: Box::new(move |_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            Duration::from_millis(u64::from(30 + (state >> 8) % spread_ms.max(1)))
        }),
        lost: Box::new(|_| false),
        sender_speed: 1.0,
    }
}

#[test]
fn every_device_quantum_is_fed() {
    for quantum in [256, 1024, 2048, 4096] {
        let run = simulate_device(
            jittery(3, 20),
            Device::with_quantum(quantum),
            PACKETS,
            silent_tail(PACKETS),
            SECONDS,
        );

        let lasting = run.underruns.iter().filter(|at| **at > LEARNING).count();
        assert_eq!(lasting, 0, "a {quantum}-frame device keeps starving");
        assert_eq!(
            run.counts.decoded.get(),
            PACKETS,
            "packets went unplayed at {quantum}"
        );
        assert_eq!(
            count_glitches(&run.played),
            0,
            "the sound breaks at {quantum}"
        );
    }
}

#[test]
fn late_packets_play_instead_of_concealment() {
    for spread in [60, 120] {
        let run = simulate_device(
            jittery(7, spread),
            Device::with_quantum(1024),
            PACKETS,
            silent_tail(PACKETS),
            SECONDS,
        );

        assert_eq!(
            run.counts.decoded.get(),
            PACKETS,
            "a late packet was dropped under {spread} ms of jitter"
        );
        let span = speech_span(&run.speech);
        let concealed = span
            .iter()
            .filter(|kind| **kind == SpeechType::Concealed)
            .count();
        assert!(
            concealed * 100 <= span.len() * 2,
            "{concealed} of {} chunks were concealed under {spread} ms of jitter",
            span.len()
        );
    }
}

#[test]
fn clean_network_never_conceals() {
    let run = simulate_device(
        Network::steady(),
        Device::with_quantum(1024),
        PACKETS,
        silent_tail(PACKETS),
        SECONDS,
    );

    let span = speech_span(&run.speech);
    assert!(span.iter().all(|kind| *kind == SpeechType::Speech));
    assert!(run.underruns.is_empty());
    assert_eq!(count_glitches(&run.played), 0);
}
