use std::time::{Duration, Instant};

use rikka_animation::FrameClock;

const PERIOD: Duration = Duration::from_micros(5_556);

const JITTER: [u64; 8] = [0, 900, 150, 1_400, 300, 0, 1_100, 600];

#[test]
fn jittered_callbacks_present_at_steady_period() {
    let start = Instant::now();
    let mut clock = FrameClock::new();
    let shown: Vec<Instant> = (0..240u32)
        .map(|frame| {
            let late = Duration::from_micros(JITTER[frame as usize % JITTER.len()]);
            clock.presenting(start + PERIOD * frame + late, PERIOD)
        })
        .collect();
    let steps: Vec<f64> = shown
        .windows(2)
        .skip(32)
        .map(|pair| pair[1].duration_since(pair[0]).as_secs_f64())
        .collect();
    let worst = steps
        .iter()
        .map(|step| (step - PERIOD.as_secs_f64()).abs())
        .fold(0.0, f64::max);
    assert!(
        worst < PERIOD.as_secs_f64() * 0.05,
        "frame deviated {worst} s from period"
    );
}

#[test]
fn stall_presents_next_frame_one_period_later() {
    let start = Instant::now();
    let mut clock = FrameClock::new();
    assert_eq!(clock.presenting(start, PERIOD), start + PERIOD);
    clock.presenting(start + PERIOD, PERIOD);
    let resumed = start + PERIOD * 9;
    assert_eq!(clock.presenting(resumed, PERIOD), resumed + PERIOD);
}

#[test]
fn early_frame_presents_one_period_after_last() {
    let start = Instant::now();
    let mut clock = FrameClock::new();
    let first = clock.presenting(start, PERIOD);
    let early = clock.ahead(start + PERIOD / 4, PERIOD);
    assert_eq!(early, first + PERIOD);
    let next = clock.presenting(start + PERIOD / 3, PERIOD);
    assert!(next >= early, "{next:?} presented before {early:?}");
}

#[test]
fn vblank_feedback_snaps_frames_to_display_grid() {
    let vblank = Instant::now();
    let mut clock = FrameClock::new();
    clock.presented(vblank, Some(PERIOD));
    let shown: Vec<Instant> = (1..40u32)
        .map(|frame| {
            let late = Duration::from_micros(JITTER[frame as usize % JITTER.len()]);
            let skipped = u32::from(frame >= 20);
            clock.presenting(vblank + PERIOD * (frame + skipped) + late, PERIOD)
        })
        .collect();
    for (at, shown) in shown.iter().enumerate() {
        let frame = at as u32 + 1;
        let skipped = u32::from(frame >= 20);
        assert_eq!(
            *shown,
            vblank + PERIOD * (frame + skipped + 1),
            "frame {frame} off display grid"
        );
    }
}

#[test]
fn superseded_frames_ahead_keep_frames_on_display_time() {
    let vblank = Instant::now();
    let mut clock = FrameClock::new();
    clock.presented(vblank, Some(PERIOD));
    for frame in 0..120u32 {
        let answered = vblank + PERIOD * frame + Duration::from_micros(200);
        assert_eq!(
            clock.presenting(answered, PERIOD),
            vblank + PERIOD * (frame + 1),
            "frame {frame} drifted from the display"
        );
        clock.ahead(answered + Duration::from_micros(1_000), PERIOD);
    }
}

#[test]
fn frame_built_ahead_counts_once_committed() {
    let vblank = Instant::now();
    let mut clock = FrameClock::new();
    clock.presented(vblank, Some(PERIOD));
    let first = clock.presenting(vblank, PERIOD);
    let early = clock.ahead(vblank + PERIOD / 4, PERIOD);
    assert_eq!(early, first + PERIOD);
    clock.commit_ahead();
    let next = clock.presenting(first + PERIOD / 4, PERIOD);
    assert_eq!(next, early + PERIOD);
}

#[test]
fn variable_refresh_predicts_from_callbacks() {
    let start = Instant::now();
    let mut clock = FrameClock::new();
    clock.presented(start, None);
    assert_eq!(clock.presenting(start, PERIOD), start + PERIOD);
}
