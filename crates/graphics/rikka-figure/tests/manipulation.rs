use rikka_figure::{
    FormKind, Frame, GrabTarget, Grip, Grips, Keys, Manipulation, ManipulationResult, Shape,
    ShapeKind, constrained_span,
};
use rikka_types::Point;

const SHIFT: Keys = Keys {
    shift: true,
    alt: false,
};

const ALT: Keys = Keys {
    shift: false,
    alt: true,
};

fn frame_of(reach: ManipulationResult) -> Frame {
    match reach.grips() {
        Grips::Frame { frame, .. } => frame,
        Grips::Ends(..) => panic!("frame"),
    }
}

#[test]
fn shift_constrains_and_alt_resizes_from_center() {
    let rectangle = FormKind::Shape(ShapeKind::Rectangle);
    let anchor = Point::new(100.0, 100.0);

    assert_eq!(
        constrained_span(rectangle, anchor, Point::new(160.0, 120.0), SHIFT),
        (anchor, Point::new(160.0, 160.0))
    );
    assert_eq!(
        constrained_span(rectangle, anchor, Point::new(160.0, 120.0), ALT),
        (Point::new(40.0, 80.0), Point::new(160.0, 120.0))
    );
}

#[test]
fn corner_resize_pins_opposite_corner() {
    let square = Grips::Frame {
        frame: Frame::from_corners(Point::new(0.0, 0.0), Point::new(10.0, 10.0)),
        sides: true,
        turns: true,
    };
    let manipulation = Manipulation::new(
        GrabTarget::Grip(Grip::Side(1, 1)),
        Point::new(10.0, 10.0),
        square,
    );

    let frame = frame_of(manipulation.drag_to(Point::new(20.0, 15.0), Keys::default()));
    assert!((frame.width - 20.0).abs() < 1e-3 && (frame.height - 15.0).abs() < 1e-3);
    assert!(frame.center.distance(Point::new(10.0, 7.5)) < 1e-3);

    let frame = frame_of(manipulation.drag_to(Point::new(20.0, 15.0), SHIFT));
    assert!((frame.width - frame.height).abs() < 1e-3);
}

#[test]
fn loaded_shape_is_clamped() {
    for (loaded, checked) in [
        (
            Shape::Rectangle { radius: f32::NAN },
            Shape::Rectangle { radius: 0.0 },
        ),
        (
            Shape::Star {
                points: 5,
                depth: f32::NAN,
            },
            Shape::from(ShapeKind::Star),
        ),
        (Shape::Polygon { sides: 1 }, Shape::Polygon { sides: 3 }),
        (
            Shape::Star {
                points: 500,
                depth: 2.0,
            },
            Shape::Star {
                points: 64,
                depth: 0.95,
            },
        ),
    ] {
        assert_eq!(loaded.checked(), checked, "{loaded:?}");
    }
}
