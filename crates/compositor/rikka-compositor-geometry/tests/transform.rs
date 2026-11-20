use rikka_compositor_geometry::{Logical, Point, Rect, Size, Transform};

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect<i32, Logical> {
    Rect::new((x, y).into(), (w, h).into())
}

#[test]
fn transforms_map_points_and_rects() {
    let area = Size::<i32, Logical>::from((70, 90));
    let point = Point::from((10, 20));
    let original = rect(10, 20, 30, 40);
    let expected = [
        (Transform::Normal, (10, 20), rect(10, 20, 30, 40)),
        (Transform::Rotate90, (20, 60), rect(20, 30, 40, 30)),
        (Transform::Rotate180, (60, 70), rect(30, 30, 30, 40)),
        (Transform::Rotate270, (70, 10), rect(30, 10, 40, 30)),
        (Transform::Flipped, (60, 20), rect(30, 20, 30, 40)),
        (Transform::Flipped90, (20, 10), rect(20, 10, 40, 30)),
        (Transform::Flipped180, (10, 70), rect(10, 30, 30, 40)),
        (Transform::Flipped270, (70, 60), rect(30, 30, 40, 30)),
    ];
    for (transform, moved_point, moved_rect) in expected {
        assert_eq!(
            point.transformed_in(transform, &area),
            Point::from(moved_point),
            "{transform:?}"
        );
        assert_eq!(
            original.transformed_in(transform, &area),
            moved_rect,
            "{transform:?}"
        );
    }
}
