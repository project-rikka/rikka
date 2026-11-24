use std::f32::consts::{FRAC_PI_2, PI};

use rikka_types::{Color, Gradient, Point, Rect};

fn close(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-4 && (a.y - b.y).abs() < 1e-4
}

#[test]
fn line_follows_css_gradient_line() {
    let rect = Rect::new(10.0, 20.0, 200.0, 100.0);
    let cases = [
        (0.0, Point::new(110.0, 120.0), Point::new(110.0, 20.0)),
        (FRAC_PI_2, Point::new(10.0, 70.0), Point::new(210.0, 70.0)),
        (PI, Point::new(110.0, 20.0), Point::new(110.0, 120.0)),
    ];
    for (angle, start, end) in cases {
        let (from, to) = Gradient::linear(angle, Color::BLACK, Color::WHITE).line(rect);
        assert!(close(from, start), "{angle}: {from:?} is not {start:?}");
        assert!(close(to, end), "{angle}: {to:?} is not {end:?}");
    }
}

#[test]
fn diagonal_line_reaches_corners() {
    let rect = Rect::new(0.0, 0.0, 100.0, 100.0);
    let (from, to) = Gradient::linear(PI * 0.75, Color::BLACK, Color::WHITE).line(rect);
    assert!(close(from, Point::new(0.0, 0.0)));
    assert!(close(to, Point::new(100.0, 100.0)));
}

#[test]
fn stops_stay_ordered_and_bounded() {
    let red = Color::rgba(1.0, 0.0, 0.0, 1.0);
    let mut gradient = Gradient::linear(0.0, Color::BLACK, Color::WHITE)
        .with_stop(0.5, red)
        .expect("room for a stop");
    let offsets: Vec<f32> = gradient.stops().iter().map(|stop| stop.offset).collect();
    assert_eq!(offsets, [0.0, 0.5, 1.0]);
    assert_eq!(gradient.stops()[1].color, red);
    for _ in 3..Gradient::MAX_STOPS {
        gradient = gradient.with_stop(0.25, red).expect("room for a stop");
    }
    assert!(gradient.with_stop(0.75, red).is_err());
}
