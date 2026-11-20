use std::time::Duration;

use replay_ring::Ring;

use crate::support::{SECOND, frame, sound};

const KEEP: Duration = Duration::from_secs(10);
const PLENTY: usize = usize::MAX;

fn filmed_ring(until_us: i64) -> (Ring, replay_ring::PictureId) {
    let mut ring = Ring::new(KEEP, PLENTY, 1);
    let id = ring.open_pictures();
    let mut at = 0;
    while at < until_us {
        ring.push_picture(id, frame(at, at % (2 * SECOND) == 0, 100));
        at += SECOND / 2;
    }
    (ring, id)
}

#[test]
fn trim_keeps_whole_gops_across_window() {
    for (now, kept_from) in [
        (30 * SECOND, 20 * SECOND),
        (31 * SECOND, 20 * SECOND),
        (32 * SECOND, 22 * SECOND),
        (5 * SECOND, 0),
    ] {
        let (mut ring, id) = filmed_ring(now);
        ring.trim(now);
        let span = ring.span(id).expect("pictures are held");
        assert_eq!(span.from_us, kept_from, "trimmed at {now}");
    }
}

#[test]
fn still_screen_keeps_last_picture() {
    let mut ring = Ring::new(KEEP, PLENTY, 1);
    let id = ring.open_pictures();
    ring.push_picture(id, frame(0, true, 100));
    ring.trim(100 * SECOND);
    let clip = ring
        .clip(id, 90 * SECOND..100 * SECOND)
        .expect("the still picture is cut");
    assert_eq!(clip.pictures.len(), 1);
    assert_eq!(clip.pictures[0].pts_us, 0);
    assert_eq!(clip.pictures[0].duration_us, 10 * SECOND);
    assert_eq!(clip.length_us, 10 * SECOND);
}

#[test]
fn drops_pictures_before_first_keyframe() {
    let mut ring = Ring::new(KEEP, PLENTY, 1);
    let id = ring.open_pictures();
    ring.push_picture(id, frame(0, false, 100));
    ring.push_picture(id, frame(SECOND, false, 100));
    assert_eq!(ring.span(id), None);
    ring.push_picture(id, frame(2 * SECOND, true, 100));
    assert_eq!(ring.span(id).map(|span| span.from_us), Some(2 * SECOND));
}

#[test]
fn ended_stream_dropped_outside_window() {
    let mut ring = Ring::new(KEEP, PLENTY, 1);
    let id = ring.open_pictures();
    ring.push_picture(id, frame(0, true, 100));
    ring.push_picture(id, frame(SECOND, false, 100));
    ring.end_pictures(id);
    ring.trim(5 * SECOND);
    assert!(ring.span(id).is_some_and(|span| span.ended));
    ring.trim(12 * SECOND);
    assert_eq!(ring.pictures().count(), 0);
}

#[test]
fn sound_outside_window_is_dropped() {
    let mut ring = Ring::new(KEEP, PLENTY, 1);
    for tick in 0..100 {
        ring.push_sound(0, sound(tick * SECOND / 5));
    }
    let id = ring.open_pictures();
    ring.push_picture(id, frame(15 * SECOND, true, 100));
    ring.trim(20 * SECOND);
    let clip = ring
        .clip(id, 10 * SECOND..20 * SECOND)
        .expect("a clip from the only keyframe");
    assert_eq!(clip.sounds[0].first().map(|packet| packet.pts_us), Some(0));
    assert_eq!(clip.sounds[0].len(), 25);
}
