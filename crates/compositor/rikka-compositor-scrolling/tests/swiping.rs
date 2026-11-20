use std::time::Duration;

use rikka_compositor_scrolling::Direction;

use crate::common::{laid, strip_with};

fn millis(at: u64) -> Duration {
    Duration::from_millis(at)
}

#[test]
fn slow_swipe_snaps_to_nearest_column() {
    let mut strip = strip_with(&[1, 2, 3]);
    assert!(strip.swipe_begin());
    strip.swipe_update(600.0, millis(0));
    assert_eq!(
        laid(&mut strip)[0],
        (1, [15, 10, 485, 780]),
        "the strip follows the fingers while they are down"
    );
    let speed = strip.swipe_end(millis(1000));
    assert_eq!(speed, 0.0);
    assert!(!strip.swiping());
    assert_eq!(strip.active(), Some(2));
    assert_eq!(
        laid(&mut strip),
        [
            (1, [10, 10, 485, 780]),
            (2, [505, 10, 485, 780]),
            (3, [1000, 10, 485, 780]),
        ]
    );
}

#[test]
fn fling_snaps_to_projected_column() {
    let mut strip = strip_with(&[1, 2, 3]);
    strip.focus(Direction::Left);
    strip.focus(Direction::Left);
    strip.settle();
    assert!(strip.swipe_begin());
    for step in 0..6 {
        strip.swipe_update(-30.0, millis(step * 10));
    }
    let speed = strip.swipe_end(millis(60));
    assert!(speed > 0.0, "the view was flung rightwards, at {speed}");
    assert_eq!(strip.active(), Some(3));
    assert_eq!(laid(&mut strip)[2], (3, [505, 10, 485, 780]));
}

#[test]
fn empty_strip_ignores_swipe() {
    let mut strip = strip_with(&[]);
    assert!(!strip.swipe_begin());
    assert_eq!(strip.swipe_end(millis(0)), 0.0);
}
