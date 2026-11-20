use std::time::Duration;

use replay_ring::Ring;

use crate::support::{SECOND, frame, pts, sound};

fn recorded_ring() -> (Ring, replay_ring::PictureId) {
    let mut ring = Ring::new(Duration::from_secs(60), usize::MAX, 1);
    let id = ring.open_pictures();
    for second in 0..10 {
        ring.push_picture(id, frame(second * SECOND, second % 4 == 0, 10));
    }
    for tick in 0..50 {
        ring.push_sound(0, sound(tick * SECOND / 5));
    }
    (ring, id)
}

#[test]
fn clip_starts_on_preceding_keyframe() {
    let (ring, id) = recorded_ring();
    let clip = ring.clip(id, 5 * SECOND..19 * SECOND / 2).expect("a clip");
    assert_eq!(
        pts(&clip.pictures),
        vec![0, SECOND, 2 * SECOND, 3 * SECOND, 4 * SECOND, 5 * SECOND]
    );
    assert!(clip.pictures[0].keyframe);
    assert_eq!(clip.length_us, 11 * SECOND / 2);
    assert_eq!(
        clip.pictures.last().map(|packet| packet.duration_us),
        Some(SECOND / 2)
    );
    let heard = pts(&clip.sounds[0]);
    assert_eq!(heard.first(), Some(&0));
    assert!(heard.iter().all(|at| *at < clip.length_us));
    assert_eq!(heard.len(), 28);
}

#[test]
fn empty_span_yields_no_clip() {
    let (ring, id) = recorded_ring();
    for (from, to) in [(5 * SECOND, 5 * SECOND), (6 * SECOND, 2 * SECOND)] {
        assert_eq!(ring.clip(id, from..to), None, "{from}..{to}");
    }
    let mut empty = Ring::new(Duration::from_secs(60), usize::MAX, 1);
    let nothing = empty.open_pictures();
    assert_eq!(empty.clip(nothing, 0..SECOND), None);
}

#[test]
fn span_after_last_frame_reuses_keyframe() {
    let (ring, id) = recorded_ring();
    let clip = ring
        .clip(id, 20 * SECOND..30 * SECOND)
        .expect("the last picture is still on screen");
    assert_eq!(pts(&clip.pictures), vec![0, SECOND]);
    assert_eq!(clip.length_us, 22 * SECOND);
}
