use std::time::Duration;

use replay_ring::Ring;

use crate::support::{SECOND, frame, sound};

const KEEP: Duration = Duration::from_secs(600);
const FRAME: usize = 1_000;

#[test]
fn budget_evicts_oldest_gop_first() {
    let mut ring = Ring::new(KEEP, usize::MAX, 1);
    let early = ring.open_pictures();
    let late = ring.open_pictures();
    for second in 0..6 {
        ring.push_picture(early, frame(second * SECOND, second % 2 == 0, FRAME));
        ring.push_picture(late, frame((second + 1) * SECOND, second % 3 == 0, FRAME));
    }
    let full = ring.used();
    ring.set_limits(KEEP, full - FRAME);
    ring.trim(10 * SECOND);
    assert_eq!(ring.span(early).map(|span| span.from_us), Some(2 * SECOND));
    assert_eq!(ring.span(late).map(|span| span.from_us), Some(SECOND));

    ring.set_limits(KEEP, 0);
    ring.trim(10 * SECOND);
    assert_eq!(ring.span(early).map(|span| span.from_us), Some(4 * SECOND));
    assert_eq!(ring.span(late).map(|span| span.from_us), Some(4 * SECOND));
    assert!(
        ring.used() > 0,
        "the last stretch of each subject always stays"
    );
}

#[test]
fn sound_before_pictures_is_evicted() {
    let mut ring = Ring::new(KEEP, usize::MAX, 1);
    let id = ring.open_pictures();
    for second in 0..6 {
        ring.push_picture(id, frame(second * SECOND, second % 2 == 0, FRAME));
        ring.push_sound(0, sound(second * SECOND));
    }
    ring.set_limits(KEEP, 0);
    ring.trim(6 * SECOND);
    let clip = ring
        .clip(id, 0..6 * SECOND)
        .expect("the last stretch is cut");
    assert_eq!(clip.pictures.len(), 2);
    assert_eq!(clip.sounds[0].len(), 2);
}

#[test]
fn ended_stream_evicted_before_live_stream() {
    let mut ring = Ring::new(KEEP, usize::MAX, 0);
    let gone = ring.open_pictures();
    let living = ring.open_pictures();
    ring.push_picture(gone, frame(0, true, FRAME));
    ring.push_picture(living, frame(SECOND, true, FRAME));
    ring.end_pictures(gone);
    ring.set_limits(KEEP, FRAME * 3 / 2);
    ring.trim(2 * SECOND);
    assert_eq!(ring.pictures().collect::<Vec<_>>(), vec![living]);
}
