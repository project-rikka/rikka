use std::time::Duration;

use rikka_compositor_kms::{FrameOutcome, FramePacer, Lateness, Timestamp, Timing};
use rikka_compositor_types::Pacing;

const REFRESH: Duration = Duration::from_micros(16_666);
const FLIP: Timestamp = Timestamp::from_monotonic(Duration::from_secs(10));

fn timing(lead: Duration) -> Timing {
    Timing {
        refresh: REFRESH,
        vrr: false,
        last_vblank: Some(FLIP),
        sequence: 1,
        lead,
    }
}

#[test]
fn next_vblank_is_whole_cycles_ahead() {
    let timing = timing(Duration::ZERO);
    assert_eq!(timing.next_vblank(FLIP), Some(FLIP));
    assert_eq!(
        timing.next_vblank(FLIP + Duration::from_millis(1)),
        Some(FLIP + REFRESH)
    );
    assert_eq!(
        timing.next_vblank(FLIP + REFRESH + Duration::from_millis(1)),
        Some(FLIP + REFRESH * 2)
    );
    assert_eq!(timing.next_vblank(FLIP + Duration::from_secs(2)), None);
}

#[test]
fn late_commit_targets_next_vblank() {
    let lead = Duration::from_millis(2);
    let timing = timing(lead);

    let early = FLIP + Duration::from_millis(5);
    assert_eq!(
        timing.next_deadline(early),
        Some((FLIP + REFRESH, FLIP + REFRESH - lead))
    );

    let late = FLIP + REFRESH - Duration::from_millis(1);
    assert_eq!(
        timing.next_deadline(late),
        Some((FLIP + REFRESH * 2, FLIP + REFRESH * 2 - lead))
    );
}

#[test]
fn late_gpu_frames_start_later() {
    let on_time = Lateness::new(Duration::ZERO, Duration::ZERO);
    let timing = Timing {
        last_vblank: Some(Timestamp::now() - Duration::from_millis(10)),
        ..timing(Duration::ZERO)
    };
    let aimed = |finished_by_gpu: Option<Duration>| {
        let mut pacer = FramePacer::new("test");
        for _ in 0..8 {
            let aimed = pacer.begin(REFRESH, on_time);
            pacer.finished(FrameOutcome::Committed);
            if let Some(after) = finished_by_gpu {
                pacer.rendered(after, Timestamp::now() + after, aimed);
            }
        }
        pacer.plan(&timing, Duration::ZERO);
        pacer.begin(REFRESH, on_time)
    };

    let quick = aimed(None);
    let slow = aimed(Some(Duration::from_millis(10)));
    assert!(
        slow > quick,
        "a frame the GPU needs 10 ms for aims past the vblank 6.7 ms away: {quick:?} vs {slow:?}"
    );
}

#[test]
fn single_late_frame_moves_next_start() {
    let on_time = Lateness::new(Duration::ZERO, Duration::ZERO);
    let timing = Timing {
        last_vblank: Some(Timestamp::now() - Duration::from_millis(10)),
        ..timing(Duration::ZERO)
    };
    let aimed = |last: Duration| {
        let mut pacer = FramePacer::new("test");
        for frame in 0..64 {
            let aimed = pacer.begin(REFRESH, on_time);
            pacer.finished(FrameOutcome::Committed);
            let after = match frame {
                63 => last,
                _ => Duration::from_millis(1),
            };
            pacer.rendered(after, Timestamp::now() + after, aimed);
        }
        pacer.plan(&timing, Duration::ZERO);
        pacer.begin(REFRESH, on_time)
    };

    let steady = aimed(Duration::from_millis(1));
    let spiked = aimed(Duration::from_millis(12));
    assert!(
        spiked > steady,
        "after one 12 ms frame among quick ones the next frame starts for a later vblank: {steady:?} vs {spiked:?}"
    );
}

fn paced(pacing: Pacing, cost: Duration, timing: &Timing) -> (FramePacer, Duration) {
    let on_time = Lateness::new(Duration::ZERO, Duration::ZERO);
    let mut pacer = FramePacer::new("test");
    pacer.set_pacing(pacing);
    for _ in 0..16 {
        let aimed = pacer.begin(REFRESH, on_time);
        pacer.finished(FrameOutcome::Committed);
        pacer.rendered(cost, Timestamp::now() + cost, aimed);
        pacer.discarded();
    }
    let delay = pacer.plan(timing, Duration::ZERO);
    (pacer, delay)
}

#[test]
fn pacing_decides_frames_in_flight() {
    let timing = Timing {
        last_vblank: Some(Timestamp::now() - Duration::from_millis(10)),
        ..timing(Duration::ZERO)
    };
    let cheap = Duration::from_millis(1);
    let expensive = Duration::from_millis(20);
    let cases = [
        (
            "balanced pacing draws ahead of frames longer than a refresh",
            Pacing::Balanced,
            expensive,
            2,
        ),
        (
            "balanced pacing draws cheap frames one at a time",
            Pacing::Balanced,
            cheap,
            1,
        ),
        (
            "low-latency pacing never queues a second frame",
            Pacing::LowLatency,
            expensive,
            1,
        ),
        (
            "steady pacing keeps a second frame in flight",
            Pacing::Steady,
            cheap,
            2,
        ),
        (
            "immediate pacing never queues a second frame",
            Pacing::Immediate,
            expensive,
            1,
        ),
    ];
    for (name, pacing, cost, in_flight) in cases {
        let (pacer, _) = paced(pacing, cost, &timing);
        assert_eq!(pacer.frames_in_flight(), in_flight, "{name}");
    }
}

#[test]
fn immediate_pacing_starts_without_waiting_for_vblank() {
    let timing = Timing {
        last_vblank: Some(Timestamp::now() - Duration::from_millis(1)),
        ..timing(Duration::ZERO)
    };
    let cheap = Duration::from_millis(1);
    let (_, balanced) = paced(Pacing::Balanced, cheap, &timing);
    let (_, immediate) = paced(Pacing::Immediate, cheap, &timing);
    assert!(
        balanced > Duration::from_millis(5),
        "a cheap vsynced frame waits for its vblank: {balanced:?}"
    );
    assert_eq!(immediate, Duration::ZERO);
}
