use std::time::{Duration, Instant};

use animate_cast::{Clock, frame_of, time_of};

const CLOSE: f32 = 1e-3;
const FOUR_HOURS: u32 = 30 * 60 * 60 * 4;

fn after(start: Instant, seconds: f32) -> Instant {
    start + Duration::from_secs_f32(seconds)
}

#[test]
fn frames_survive_the_trip_through_seconds() {
    for frame in 0..=FOUR_HOURS {
        assert_eq!(frame_of(time_of(frame)), frame);
    }
}

#[test]
fn clock_stops_at_the_end_unless_it_repeats() {
    let start = Instant::now();
    let mut clock = Clock::default();
    clock.run(start);
    assert!(!clock.is_running(), "an empty timeline does not run");

    clock.set_length(10.0, start);
    clock.run(start);
    assert!((clock.time(after(start, 4.0)) - 4.0).abs() < CLOSE);
    assert_eq!(clock.time(after(start, 12.0)), 10.0);
    assert!(clock.has_ended(after(start, 12.0)));

    clock.set_repeat(true, after(start, 1.0));
    assert!((clock.time(after(start, 12.0)) - 2.0).abs() < CLOSE);
    assert!(!clock.has_ended(after(start, 30.0)));
}

#[test]
fn clock_seeks_inside_while_running_and_anywhere_while_stopped() {
    let start = Instant::now();
    let mut clock = Clock::default();
    clock.set_length(10.0, start);

    clock.seek(14.0, start);
    assert_eq!(clock.time(after(start, 5.0)), 14.0);

    clock.run(start);
    assert!(
        clock.time(start) < CLOSE,
        "running from past the end starts over"
    );
    clock.seek(14.0, after(start, 1.0));
    assert_eq!(clock.time(after(start, 1.0)), 10.0);

    clock.stop(after(start, 1.0));
    clock.set_length(4.0, after(start, 1.0));
    assert_eq!(clock.time(after(start, 9.0)), 10.0);
}
