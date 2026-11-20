use crate::support::fullband::{Conversation, Talk, process};

#[test]
fn reported_delay_tracks_echo_path() {
    for delay_ms in [12, 50, 130, 250] {
        let conversation = Conversation::new(&Talk {
            frames: 500,
            delay_samples: delay_ms * 48,
            near_frames: 0..0,
        });
        let reported = process(&conversation).metrics.last().unwrap().delay_ms;
        let expected = ((delay_ms * 16 - 32) / 64 * 4) as i32;
        assert!(
            (reported - expected).abs() <= 4,
            "a {delay_ms} ms echo path reports {reported} ms, expected {expected} ms"
        );
    }
}
