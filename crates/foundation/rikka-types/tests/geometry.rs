use rikka_types::{CornerRadii, Point, Rect, RoundedRect};

const SHAPE: RoundedRect = RoundedRect {
    rect: Rect::new(10.0, 10.0, 100.0, 40.0),
    radii: CornerRadii::uniform(8.0),
};

#[test]
fn subtract_keeps_uncovered_pixels() {
    let whole = Rect::new(0.0, 0.0, 12.0, 12.0);
    let taken = Rect::new(4.0, 4.0, 4.0, 4.0);
    let mut parts = Vec::new();
    whole.subtract(&taken, &mut parts);
    for x in 0..12 {
        for y in 0..12 {
            let point = Point::new(x as f32 + 0.5, y as f32 + 0.5);
            assert_eq!(
                parts.iter().any(|part| part.contains(point)),
                !taken.contains(point),
                "{point:?}"
            );
        }
    }
    for (index, one) in parts.iter().enumerate() {
        for other in &parts[index + 1..] {
            assert!(!one.intersects(other), "{one:?} overlaps {other:?}");
        }
    }
}

#[test]
fn clip_squares_only_reached_corners() {
    assert_eq!(
        SHAPE.intersect(Rect::new(0.0, 0.0, 200.0, 200.0)),
        Some(SHAPE)
    );
    assert_eq!(SHAPE.intersect(Rect::new(300.0, 0.0, 10.0, 10.0)), None);

    let clipped = SHAPE.intersect(Rect::new(0.0, 0.0, 60.0, 200.0)).unwrap();
    assert_eq!(clipped.rect, Rect::new(10.0, 10.0, 50.0, 40.0));
    assert_eq!(clipped.radii, CornerRadii::new(8.0, 0.0, 0.0, 8.0));
}

#[test]
fn inset_corners_stay_concentric() {
    let inset = RoundedRect::rect(Rect::new(11.0, 13.0, 50.0, 30.0));
    let clipped = inset.intersect(SHAPE).unwrap();
    assert_eq!(clipped.radii, CornerRadii::new(5.0, 0.0, 0.0, 1.0));

    let square = RoundedRect::rect(Rect::new(10.0, 10.0, 100.0, 20.0));
    let shared = square.intersect(SHAPE).unwrap();
    assert_eq!(shared.radii, CornerRadii::new(8.0, 8.0, 0.0, 0.0));
    assert_eq!(SHAPE.intersect(square), Some(shared));
}

#[test]
fn changed_corners_reports_moved_radii() {
    let window = Rect::new(0.0, 0.0, 400.0, 300.0);
    let rounded = RoundedRect::new(window, 16.0);
    let reach = 16.0 * RoundedRect::CORNER_REACH + 1.0;
    let cases = [
        (
            RoundedRect::new(window, 0.0),
            vec![
                Rect::new(0.0, 0.0, reach, reach),
                Rect::new(400.0 - reach, 0.0, reach, reach),
                Rect::new(400.0 - reach, 300.0 - reach, reach, reach),
                Rect::new(0.0, 300.0 - reach, reach, reach),
            ],
        ),
        (
            RoundedRect::new(window, CornerRadii::new(16.0, 16.0, 4.0, 16.0)),
            vec![Rect::new(400.0 - reach, 300.0 - reach, reach, reach)],
        ),
        (rounded, vec![]),
    ];
    for (to, expected) in cases {
        let corners: Vec<Rect> = to.changed_corners(&rounded).unwrap().collect();
        assert_eq!(corners, expected, "{to:?}");
    }
    let moved = RoundedRect::new(Rect::new(1.0, 0.0, 400.0, 300.0), 0.0);
    assert!(moved.changed_corners(&rounded).is_none());
}

#[test]
fn rim_covers_sides_and_corners() {
    let rim: Vec<Rect> = SHAPE.rim(1.0).collect();
    let reach = 8.0 * RoundedRect::CORNER_REACH + 1.0;
    assert_eq!(rim.len(), 8);
    assert_eq!(rim[0], Rect::new(10.0, 10.0, 100.0, 2.0));
    assert_eq!(rim[3], Rect::new(108.0, 10.0, 2.0, 40.0));
    assert_eq!(rim[6], Rect::new(110.0 - reach, 50.0 - reach, reach, reach));
    for x in 10..110 {
        for y in 10..50 {
            let point = Point::new(x as f32 + 0.5, y as f32 + 0.5);
            let inner = Rect::new(12.0, 12.0, 96.0, 36.0).contains(point)
                && !rim[4..].iter().any(|corner| corner.contains(point));
            assert_eq!(
                rim.iter().any(|part| part.contains(point)),
                !inner,
                "{point:?}"
            );
        }
    }
}
