use desk_layout::Projection;
use rikka_types::{Point, Rect};

#[test]
fn projection_maps_points_both_ways() {
    let projection = Projection::new(
        Rect::new(0.0, 0.0, 1920.0, 1080.0),
        Rect::new(200.0, 150.0, 960.0, 540.0),
    );
    assert_eq!(projection.scale(), 0.5);
    for desk in [
        Point::new(0.0, 0.0),
        Point::new(1920.0, 1080.0),
        Point::new(613.0, 47.5),
    ] {
        let shown = projection.point(desk);
        let back = projection.back(shown);
        assert!((back.x - desk.x).abs() < 1e-3 && (back.y - desk.y).abs() < 1e-3);
    }
    assert_eq!(
        projection.point(Point::new(100.0, 40.0)),
        Point::new(250.0, 170.0)
    );
    assert_eq!(
        projection.rect(Rect::new(100.0, 40.0, 352.0, 168.0)),
        Rect::new(250.0, 170.0, 176.0, 84.0)
    );
    assert_eq!(projection.back_length(8.0), 16.0);
}

#[test]
fn collapsed_card_maps_nothing_back() {
    let whole = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    let projection = Projection::new(Rect::new(0.0, 0.0, 0.0, 0.0), whole);
    assert_eq!(projection.scale(), 0.0);
    assert_eq!(projection.back(Point::new(10.0, 10.0)), Point::ZERO);
    assert_eq!(projection.back_length(8.0), 0.0);
}
