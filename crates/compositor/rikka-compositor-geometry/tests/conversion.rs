use rikka_compositor_geometry::{Buffer, Logical, Point, Rect, Scale, Size, Transform};

#[test]
fn precise_conversions_round_as_requested() {
    let point = Point::<i32, Logical>::from((3, 5));
    assert_eq!(
        point.to_physical_precise_round::<f64, i32>(1.25),
        Point::from((4, 6))
    );
    assert_eq!(
        point.to_physical_precise_floor::<f64, i32>(1.25),
        Point::from((3, 6))
    );
    assert_eq!(
        point.to_physical_precise_ceil::<f64, i32>(1.25),
        Point::from((4, 7))
    );
    assert_eq!(point.to_physical(1), Point::from((3, 5)));

    let size = Size::<i32, Logical>::from((3, 5));
    assert_eq!(
        size.to_physical_precise_round::<f64, i32>(1.5),
        Size::from((5, 8))
    );
    assert_eq!(
        size.to_physical_precise_floor::<f64, i32>(1.5),
        Size::from((4, 7))
    );
    assert_eq!(
        size.to_physical_precise_ceil::<f64, i32>(1.5),
        Size::from((5, 8))
    );
}

#[test]
fn precise_rects_round_to_whole_pixels() {
    let rect = Rect::<i32, Logical>::new((1, 1).into(), (3, 3).into());
    let scale = Scale::from(1.5);
    assert_eq!(
        rect.to_physical_precise_round::<f64, i32>(scale),
        Rect::new((2, 2).into(), (5, 5).into())
    );
    assert_eq!(
        rect.to_physical_precise_down::<f64, i32>(scale),
        Rect::new((2, 2).into(), (4, 4).into())
    );
    assert_eq!(
        rect.to_physical_precise_up::<f64, i32>(scale),
        Rect::new((1, 1).into(), (5, 5).into())
    );
}

#[test]
fn logical_to_buffer_scaled_and_rotated() {
    let area = Size::<i32, Logical>::from((100, 50));
    let rect = Rect::<i32, Logical>::new((10, 5).into(), (20, 10).into());
    let buffer = rect.to_buffer(2, Transform::Rotate90, &area);
    assert_eq!(
        buffer,
        Rect::<i32, Buffer>::new((10, 140).into(), (20, 40).into())
    );

    let buffer_area = area.to_buffer(2, Transform::Rotate90);
    assert_eq!(buffer_area, Size::from((100, 200)));
    assert_eq!(
        buffer.to_logical(2, Transform::Rotate90, &buffer_area),
        rect
    );

    let point = Point::<i32, Logical>::from((10, 5));
    let buffer_point = point.to_buffer(2, Transform::Rotate90, &area);
    assert_eq!(buffer_point, Point::<i32, Buffer>::from((10, 180)));
    assert_eq!(
        buffer_point.to_logical(2, Transform::Rotate90, &buffer_area),
        point
    );
    assert_eq!(buffer_area.to_logical(2, Transform::Rotate90), area);
}

#[test]
fn buffer_round_trip_every_transform() {
    let area = Size::<i32, Logical>::from((100, 50));
    let rect = Rect::<i32, Logical>::new((10, 5).into(), (20, 10).into());
    for transform in Transform::ALL {
        let buffer_area = area.to_buffer(2, transform);
        let buffer = rect.to_buffer(2, transform, &buffer_area.to_logical(2, transform));
        assert_eq!(
            buffer.to_logical(2, transform, &buffer_area),
            rect,
            "{transform:?}"
        );
    }
}
