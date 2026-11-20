use rikka_types::{Point, Rect, Size, Transform};

const AREA: Size = Size::new(70.0, 90.0);

const POINT: Point = Point::new(10.0, 20.0);

#[test]
fn quarter_turn_is_counter_clockwise() {
    let top_right = Point::new(AREA.width, 0.0);
    assert_eq!(
        Transform::Rotate90.map_point(top_right, AREA),
        Point::new(0.0, 0.0)
    );
    assert_eq!(
        Transform::Rotate90.map_point(Point::new(0.0, 0.0), AREA),
        Point::new(0.0, AREA.width)
    );
}

#[test]
fn flipped_transforms_mirror_then_turn() {
    let mirrored = Transform::Flipped.map_point(POINT, AREA);
    assert_eq!(mirrored, Point::new(60.0, 20.0));
    for transform in Transform::ALL {
        let turned = Transform::ALL[transform.quarter_turns() as usize];
        let stepwise = match transform.is_flipped() {
            true => turned.map_point(mirrored, AREA),
            false => turned.map_point(POINT, AREA),
        };
        assert_eq!(transform.map_point(POINT, AREA), stepwise, "{transform:?}");
    }
}

#[test]
fn composition_applies_left_first() {
    let rect = Rect::new(10.0, 20.0, 30.0, 40.0);
    for first in Transform::ALL {
        for second in Transform::ALL {
            let stepwise = second.map_rect(first.map_rect(rect, AREA), first.map_size(AREA));
            assert_eq!(
                first.then(second).map_rect(rect, AREA),
                stepwise,
                "{first:?} then {second:?}"
            );
        }
        assert_eq!(first.then(first.inverse()), Transform::Normal);
        assert_eq!(Transform::from_wayland(first.to_wayland()), Some(first));
    }
}
