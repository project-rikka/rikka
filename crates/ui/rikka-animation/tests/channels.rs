use std::time::Duration;

use rikka_animation::{Channel, Curve, DurationToken, Easing, Spring};

const FRAME: Duration = Duration::from_micros(16_667);

#[test]
fn tween_ends_at_duration() {
    let curve = Curve::Tween {
        easing: Easing::Standard,
        duration: DurationToken::Short4,
    };
    let mut channel = Channel::new(0.0);
    channel.advance(curve, 10.0, Duration::from_millis(100));
    assert!(channel.value > 0.0 && channel.value < 10.0);
    channel.advance(curve, 10.0, Duration::from_millis(100));
    assert_eq!(channel.value, 10.0);
    assert!(channel.is_resting());
}

#[test]
fn spring_settles_and_stays_on_target() {
    let curve = Curve::Spring(Spring::FAST_SPATIAL);
    let mut channel = Channel::new(0.0);
    for _ in 0..600 {
        channel.advance(curve, 1.0, FRAME);
    }
    assert!(channel.is_resting());
    assert_eq!(channel.value, 1.0);
}

#[test]
fn distant_spring_settles_at_any_step() {
    let cases = [
        (1280.5, 1280.0, Spring::DEFAULT_SPATIAL, 100),
        (1208.0, 1207.0, Spring::DEFAULT_SPATIAL, 100),
        (145.0, 146.0, Spring::DEFAULT_SPATIAL, 10),
        (40_000.0, 39_990.0, Spring::FAST_SPATIAL, 250),
    ];
    for (from, to, spring, micros) in cases {
        let step = Duration::from_micros(micros);
        let mut channel = Channel::new(from);
        channel.advance(Curve::Spring(spring), to, step);
        let mut steps = 1;
        while !channel.is_resting() && steps * micros < 3_000_000 {
            channel.advance(Curve::Spring(spring), to, step);
            steps += 1;
        }
        assert!(
            channel.is_resting(),
            "{from} → {to} with {micros} µs steps still moving after 3 s at {}",
            channel.value
        );
        assert_eq!(channel.value, to);
    }
}
