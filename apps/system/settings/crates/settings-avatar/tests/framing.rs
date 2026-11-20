use rikka_image::Region;
use rikka_types::Point;
use settings_avatar::{Framing, MAX_ZOOM};

#[test]
fn crop_stays_a_square_inside_the_photo() {
    let start = Framing::new(400, 300);
    let cases = [
        (start, Region::new(50, 0, 300, 300)),
        (
            start.panned(Point::new(1000.0, 1000.0)),
            Region::new(100, 0, 300, 300),
        ),
        (
            start.panned(Point::new(-1000.0, 0.0)),
            Region::new(0, 0, 300, 300),
        ),
        (
            start.zoomed(2.0, Point::new(0.0, 0.0)),
            Region::new(25, 0, 150, 150),
        ),
        (
            start.zoomed(2.0, Point::new(400.0, 300.0)),
            Region::new(225, 150, 150, 150),
        ),
        (
            start.zoomed(0.25, Point::new(0.0, 0.0)),
            Region::new(50, 0, 300, 300),
        ),
    ];
    for (framing, region) in cases {
        assert_eq!(framing.region(), region, "{framing:?}");
    }
}

#[test]
fn zoom_is_bounded_and_keeps_the_anchor_in_place() {
    let start = Framing::new(1000, 1000);
    assert_eq!(start.zoomed(100.0, start.center()).zoom(), MAX_ZOOM);
    assert_eq!(start.zoomed(0.1, start.center()).zoom(), 1.0);

    let anchor = Point::new(600.0, 450.0);
    let zoomed = start.zoomed(2.0, anchor);
    let place = |framing: Framing| {
        (
            (anchor.x - framing.center().x) / framing.side(),
            (anchor.y - framing.center().y) / framing.side(),
        )
    };
    assert_eq!(place(zoomed), place(start));
}
