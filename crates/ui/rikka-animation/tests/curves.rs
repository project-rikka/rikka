use std::time::Duration;

use rikka_animation::{Channel, Easing, Spring};

const EASINGS: [Easing; 7] = [
    Easing::Emphasized,
    Easing::EmphasizedDecel,
    Easing::EmphasizedAccel,
    Easing::Standard,
    Easing::StandardDecel,
    Easing::StandardAccel,
    Easing::Linear,
];

const SPRINGS: [Spring; 6] = [
    Spring::FAST_SPATIAL,
    Spring::DEFAULT_SPATIAL,
    Spring::SLOW_SPATIAL,
    Spring::FAST_EFFECTS,
    Spring::DEFAULT_EFFECTS,
    Spring::SLOW_EFFECTS,
];

const FRAME: Duration = Duration::from_micros(16_667);

fn flight(spring: Spring) -> Vec<f32> {
    let mut channel = Channel::new(0.0);
    (0..240)
        .map(|_| {
            spring.step(&mut channel, 1.0, FRAME);
            channel.value
        })
        .collect()
}

#[test]
fn easings_are_monotonic_from_zero_to_one() {
    for easing in EASINGS {
        assert_eq!(easing.transform(0.0), 0.0, "{easing:?}");
        assert_eq!(easing.transform(1.0), 1.0, "{easing:?}");
        assert_eq!(easing.transform(-1.0), 0.0, "{easing:?}");
        assert_eq!(easing.transform(2.0), 1.0, "{easing:?}");
        let mut previous = 0.0;
        for step in 0..=200 {
            let value = easing.transform(step as f32 / 200.0);
            assert!(
                value >= previous - 1e-4,
                "{easing:?} not monotonic at {step}"
            );
            previous = value;
        }
    }
}

#[test]
fn springs_settle_on_target() {
    for spring in SPRINGS {
        let mut channel = Channel::new(0.0);
        spring.step(&mut channel, 1.0, Duration::from_secs(4));
        assert!((channel.value - 1.0).abs() < 1e-3, "{spring:?}");
        assert!(channel.velocity.abs() < 1e-2, "{spring:?}");
    }
}

#[test]
fn spatial_springs_overshoot_effects_springs_do_not() {
    for spring in SPRINGS {
        let peak = flight(spring).into_iter().fold(0.0, f32::max);
        if spring.overshoots() {
            assert!(peak > 1.0, "{spring:?} peaked at {peak}");
        } else {
            assert!(peak <= 1.0 + 1e-4, "{spring:?} peaked at {peak}");
        }
    }
    assert!(Spring::DEFAULT_SPATIAL.overshoots());
    assert!(!Spring::DEFAULT_EFFECTS.overshoots());
}

#[test]
fn spring_step_size_does_not_change_result() {
    let spring = Spring::DEFAULT_SPATIAL;
    let mut stepped = Channel::new(0.0);
    for _ in 0..30 {
        spring.step(&mut stepped, 1.0, FRAME);
    }
    let mut leapt = Channel::new(0.0);
    spring.step(&mut leapt, 1.0, FRAME * 30);
    assert!((stepped.value - leapt.value).abs() < 1e-3);
}

#[test]
fn launched_critical_spring_never_overshoots() {
    let spring = Spring::new(1.0, 440.0);
    let mut channel = Channel::new(0.0);
    channel.velocity = spring.launch_speed(1.0);
    let mut previous = (0.0, f32::INFINITY);
    for _ in 0..60 {
        spring.step(&mut channel, 1.0, FRAME / 3);
        let travelled = channel.value - previous.0;
        assert!(travelled > 0.0 && travelled <= previous.1, "{channel:?}");
        assert!(channel.value <= 1.0, "{channel:?}");
        previous = (channel.value, travelled);
    }
    let mut resting = Channel::new(0.0);
    spring.step(&mut resting, 1.0, FRAME / 3);
    let mut launched = Channel::new(0.0);
    launched.velocity = spring.launch_speed(1.0);
    spring.step(&mut launched, 1.0, FRAME / 3);
    assert!(launched.value > resting.value * 10.0);
}
