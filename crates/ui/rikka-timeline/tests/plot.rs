use rikka_timeline::CurvePlot;
use rikka_types::{Point, Rect};

#[test]
fn knob_at_picks_the_nearest_knob_within_reach() {
    let plot = CurvePlot::unit(Rect::new(0.0, 0.0, 100.0, 100.0));
    let knobs = [Point::new(0.2, 0.2), Point::new(0.3, 0.2)];
    let cases = [
        (Point::new(21.0, 80.0), Some(0)),
        (Point::new(29.0, 79.0), Some(1)),
        (Point::new(25.5, 80.0), Some(1)),
        (Point::new(20.0, 93.0), None),
        (Point::new(60.0, 20.0), None),
    ];
    for (pointer, expected) in cases {
        assert_eq!(plot.knob_at(&knobs, pointer), expected, "{pointer:?}");
    }
}
