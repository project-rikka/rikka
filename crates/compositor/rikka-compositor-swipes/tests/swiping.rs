use std::time::Duration;

use rikka_compositor_geometry::Point;
use rikka_compositor_swipes::{Outcome, Pointed, Scroll, Swiping};
use rikka_compositor_types::{Gesture, OutputId};
use rikka_session::gestures::{Axis, FINGERS, Swipe};

fn output() -> OutputId {
    slotmap::KeyData::from_ffi(1).into()
}

fn at(millis: u64) -> Duration {
    Duration::from_millis(millis)
}

fn begin(fingers: u32) -> Gesture {
    Gesture::SwipeBegin {
        fingers,
        time: at(0),
    }
}

fn update(dx: f64, dy: f64, millis: u64) -> Gesture {
    Gesture::SwipeUpdate {
        delta: Point::new(dx, dy),
        time: at(millis),
    }
}

fn end(millis: u64) -> Gesture {
    Gesture::SwipeEnd {
        cancelled: false,
        time: at(millis),
    }
}

#[derive(Clone, Copy)]
struct Place {
    announcing: bool,
    sideways_kept: bool,
    scrolls: bool,
}

fn pointed(place: Place) -> Pointed {
    Pointed {
        announcing: place.announcing,
        sideways_kept: place.sideways_kept,
        scrolls_on: place.scrolls.then(output),
        output: Some("DP-1".to_owned()),
    }
}

fn run(gestures: &[Gesture], place: Place) -> Outcome {
    let mut swiping = Swiping::default();
    let mut all = Outcome::default();
    for gesture in gestures {
        let outcome = swiping.handle(*gesture, || pointed(place));
        all.announced.extend(outcome.announced);
        all.passed.extend(outcome.passed);
        all.scrolled.extend(outcome.scrolled);
    }
    all
}

#[test]
fn swipes_go_to_the_desktop_the_strip_or_the_client() {
    let desktop = Place {
        announcing: true,
        sideways_kept: false,
        scrolls: false,
    };
    let overview = Place {
        sideways_kept: true,
        ..desktop
    };
    let strip = Place {
        announcing: false,
        sideways_kept: false,
        scrolls: true,
    };
    let vertical = [begin(FINGERS), update(1.0, -20.0, 10), end(20)];
    let sideways = [begin(FINGERS), update(-20.0, 1.0, 10), end(20)];
    let two_fingers = [begin(2), update(-20.0, 1.0, 10), end(20)];

    let announced = run(&vertical, desktop);
    assert_eq!(
        announced.announced,
        [
            Swipe::Began {
                axis: Axis::Vertical,
                output: Some("DP-1".to_owned()),
            },
            Swipe::Moved {
                dx: 1.0,
                dy: -20.0,
                time: 10,
            },
            Swipe::Ended {
                cancelled: false,
                time: 20,
            },
        ]
    );
    assert!(announced.passed.is_empty() && announced.scrolled.is_empty());

    let kept = run(&sideways, overview);
    assert_eq!(
        kept.announced.first(),
        Some(&Swipe::Began {
            axis: Axis::Horizontal,
            output: Some("DP-1".to_owned()),
        })
    );

    let scrolled = run(&sideways, strip);
    assert_eq!(
        scrolled.scrolled,
        [
            Scroll::Began { output: output() },
            Scroll::Moved {
                dx: -20.0,
                time: at(10),
            },
            Scroll::Ended { time: at(20) },
        ]
    );
    assert!(scrolled.announced.is_empty() && scrolled.passed.is_empty());

    for (gestures, place) in [
        (&sideways, desktop),
        (&two_fingers, desktop),
        (&vertical, strip),
    ] {
        let passed = run(gestures, place);
        assert_eq!(passed.passed, gestures.to_vec());
        assert!(passed.announced.is_empty() && passed.scrolled.is_empty());
    }
}

#[test]
fn short_swipe_passes_what_it_buffered() {
    let desktop = Place {
        announcing: true,
        sideways_kept: false,
        scrolls: false,
    };
    let short = [begin(FINGERS), update(2.0, 3.0, 10), end(20)];
    let outcome = run(&short, desktop);
    assert_eq!(outcome.passed, short.to_vec());
    assert!(outcome.announced.is_empty());
}

#[test]
fn new_swipe_cancels_the_one_it_interrupts() {
    let desktop = Place {
        announcing: true,
        sideways_kept: false,
        scrolls: false,
    };
    let interrupted = [begin(FINGERS), update(0.0, -30.0, 10), begin(FINGERS)];
    let outcome = run(&interrupted, desktop);
    assert_eq!(
        outcome.announced.last(),
        Some(&Swipe::Ended {
            cancelled: true,
            time: 0,
        })
    );
}
