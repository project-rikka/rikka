use std::time::Duration;

use rikka_animation::{Curve, DurationToken, Easing, Presence, Spring};

const FRAME: Duration = Duration::from_millis(10);

const LINEAR: Curve = Curve::Tween {
    easing: Easing::Linear,
    duration: DurationToken::Short4,
};

fn run(presence: &mut Presence, shown: bool, curve: Curve, frames: usize) {
    for _ in 0..frames {
        presence.advance(shown, curve, FRAME);
    }
}

#[test]
fn reversed_tween_returns_in_the_time_it_travelled() {
    for (opened, closed, settles) in [(15, 5, 10), (20, 5, 5), (5, 2, 17)] {
        let mut presence = Presence::HIDDEN;
        run(&mut presence, true, LINEAR, opened);
        run(&mut presence, false, LINEAR, closed);
        let before = presence.progress();
        presence.advance(true, LINEAR, FRAME);
        let step = presence.progress() - before;
        assert!(
            (step - 0.05).abs() < 1e-3,
            "opened {opened}, closed {closed}: reopening moved {step} in one frame from {before}"
        );
        run(&mut presence, true, LINEAR, settles - 2);
        assert!(
            !presence.is_resting(),
            "opened {opened}, closed {closed}: settled early at {}",
            presence.progress()
        );
        run(&mut presence, true, LINEAR, 2);
        assert!(
            presence.is_resting() && presence.progress() == 1.0,
            "opened {opened}, closed {closed}: still at {} after {settles} frames",
            presence.progress()
        );
    }
}

#[test]
fn reversed_spring_keeps_its_momentum() {
    let curve = Curve::Spring(Spring::DEFAULT_SPATIAL);
    let mut presence = Presence::HIDDEN;
    run(&mut presence, true, curve, 6);
    let progress = presence.progress();
    assert!(progress > 0.0 && progress < 1.0);

    presence.advance(false, curve, Duration::ZERO);
    assert_eq!(presence.progress(), progress, "reversing jumped");
    presence.advance(false, curve, FRAME);
    assert!(
        presence.progress() > progress,
        "reversed spring stopped dead at {progress} instead of carrying on to {}",
        presence.progress()
    );
    run(&mut presence, false, curve, 300);
    assert!(presence.is_resting() && presence.progress() == 0.0);
}
