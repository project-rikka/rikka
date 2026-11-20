use rikka_timeline::{ScrollExtent, ScrollTrack};

fn track(at: f64, shown: f64, content: f64) -> ScrollTrack {
    ScrollTrack {
        extent: ScrollExtent { at, shown, content },
        from: 10.0,
        length: 200.0,
    }
}

#[test]
fn thumb_shows_the_shown_share_but_never_below_its_shortest() {
    assert_eq!(track(0.0, 50.0, 100.0).thumb().length, 100.0);
    assert_eq!(track(0.0, 1.0, 10_000.0).thumb().length, 24.0);
    assert_eq!(track(0.0, 100.0, 50.0).thumb().length, 200.0);
}

#[test]
fn thumb_travels_the_track_from_start_to_end() {
    let start = track(0.0, 50.0, 100.0).thumb();
    let end = track(50.0, 50.0, 100.0).thumb();
    assert_eq!(start.start, 10.0);
    assert_eq!(end.start + end.length, 210.0);
}

#[test]
fn sliding_the_thumb_reads_back_the_scrolled_position() {
    for at in [0.0, 12.5, 37.0, 50.0] {
        let track = track(at, 50.0, 100.0);
        assert!(
            (track.at_for(track.thumb().start) - at).abs() < 1e-3,
            "{at}"
        );
    }
    let held = track(25.0, 50.0, 100.0);
    let thumb = held.thumb();
    assert_eq!(held.grip(thumb.start + 4.0), 4.0);
    assert_eq!(
        held.grip(thumb.start + thumb.length + 30.0),
        thumb.length / 2.0
    );
}
