use rikka_timeline::{TimeScale, ZoomRange};

const RANGE: ZoomRange = ZoomRange {
    farthest: 0.05,
    nearest: 48.0,
};

fn scale(width: f32, end: f64) -> TimeScale {
    let mut scale = TimeScale::new(4.0, RANGE);
    scale.set_width(width);
    scale.set_end(end);
    scale
}

#[test]
fn zooming_keeps_the_frame_under_the_pointer() {
    let mut scale = scale(800.0, 10_000.0);
    scale.scroll_to(300.0);
    for (factor, around) in [(1.2, 250.0), (0.5, 40.0), (3.0, 799.0)] {
        let pinned = scale.frame_at(around);
        scale.zoom_at(factor, around);
        assert!(
            (scale.frame_at(around) - pinned).abs() < 1e-9,
            "{factor} at {around}"
        );
    }
}

#[test]
fn zoom_stays_within_its_range() {
    let mut scale = scale(800.0, 10_000.0);
    scale.zoom_notches(-200.0, 0.0);
    assert_eq!(scale.per_frame(), RANGE.farthest);
    scale.zoom_notches(200.0, 0.0);
    assert_eq!(scale.per_frame(), RANGE.nearest);
    scale.set_level(0.0, 0.0);
    assert!((scale.per_frame() - RANGE.farthest).abs() < 1e-12);
    scale.set_level(1.0, 0.0);
    assert!((scale.level() - 1.0).abs() < 1e-6);
}

#[test]
fn scrolling_stops_at_the_start_and_half_a_view_past_the_end() {
    let mut scale = scale(400.0, 1_000.0);
    scale.scroll_by(-50.0);
    assert_eq!(scale.first(), 0.0);
    scale.scroll_by(1e9);
    assert_eq!(scale.first(), 1_000.0 - scale.span() / 2.0);
    let mut short = self::scale(400.0, 50.0);
    short.scroll_by(1e9);
    assert_eq!(short.first(), 0.0);
    assert!(!short.extent().scrolls());
}

#[test]
fn fit_shows_the_whole_length_from_the_start() {
    let mut scale = scale(1_000.0, 900.0);
    scale.scroll_to(200.0);
    scale.fit();
    assert_eq!(scale.first(), 0.0);
    assert!(scale.last() >= 900.0);
    assert!(scale.last() < 900.0 * 1.1);
}

#[test]
fn following_pages_ahead_only_when_the_frame_leaves_the_view() {
    let mut scale = scale(400.0, 10_000.0);
    scale.follow(50.0);
    assert_eq!(scale.first(), 0.0);
    scale.follow(500.0);
    assert!(scale.first() < 500.0 && scale.last() > 500.0);
    let shown = scale.first();
    scale.follow(scale.first() + 1.0);
    assert_eq!(scale.first(), shown);
}

#[test]
fn x_and_frame_are_inverse() {
    let mut scale = scale(640.0, 5_000.0);
    scale.zoom_at(2.5, 100.0);
    scale.scroll_to(123.0);
    for frame in [123.0, 150.5, 300.0] {
        assert!((scale.frame_at(scale.x_of(frame)) - frame).abs() < 1e-3);
    }
}
