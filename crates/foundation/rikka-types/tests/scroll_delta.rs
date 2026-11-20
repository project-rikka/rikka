use rikka_types::{Point, SCROLL_LINE, ScrollDelta};

struct Case {
    name: &'static str,
    delta: ScrollDelta,
    offset: Point,
    up: f32,
    right: f32,
    zoom: f32,
}

#[test]
fn directions_follow_the_content_and_the_hand_separately() {
    let line = SCROLL_LINE;
    let cases = [
        Case {
            name: "wheel rolled away",
            delta: ScrollDelta::from_up(1.0),
            offset: Point::new(0.0, -line),
            up: 1.0,
            right: 0.0,
            zoom: 1.0,
        },
        Case {
            name: "wheel rolled towards the person",
            delta: ScrollDelta::from_offset(Point::new(0.0, 2.0 * line)),
            offset: Point::new(0.0, 2.0 * line),
            up: -2.0,
            right: 0.0,
            zoom: -2.0,
        },
        Case {
            name: "wheel rolled away under natural scrolling",
            delta: ScrollDelta::from_offset(Point::new(0.0, line)).with_natural(true),
            offset: Point::new(0.0, line),
            up: 1.0,
            right: 0.0,
            zoom: 1.0,
        },
        Case {
            name: "fingers dragging the content right and slightly down",
            delta: ScrollDelta::from_content(Point::new(3.0 * line, line)).with_natural(true),
            offset: Point::new(-3.0 * line, -line),
            up: -1.0,
            right: 3.0,
            zoom: 3.0,
        },
        Case {
            name: "traditional touchpad pushing the view right",
            delta: ScrollDelta::from_offset(Point::new(line, 0.0)),
            offset: Point::new(line, 0.0),
            up: 0.0,
            right: 1.0,
            zoom: 1.0,
        },
    ];
    for case in cases {
        let Case {
            name,
            delta,
            offset,
            up,
            right,
            zoom,
        } = case;
        assert_eq!(delta.offset(), offset, "{name}");
        assert_eq!(delta.content(), -offset, "{name}");
        assert_eq!(delta.up(), up, "{name}");
        assert_eq!(delta.right(), right, "{name}");
        assert_eq!(delta.zoom(), zoom, "{name}");
    }
}
