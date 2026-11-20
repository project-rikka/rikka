use rikka_compositor_geometry::{Point, Rect, Size};
use rikka_compositor_input::{PointerRestrictions, PointerRoute, PointerShared, RouteOutput};
use rikka_compositor_types::OutputId;

fn output(x: i32, y: i32, width: i32, height: i32) -> RouteOutput {
    RouteOutput {
        area: Rect::new(Point::new(x, y), Size::new(width, height)),
        scale: 1.0,
        cursor: Some(OutputId::default()),
    }
}

#[test]
fn offscreen_point_clamps_to_nearest_edge() {
    let shared = PointerShared::default();
    let outside = Point::new(2500.0, 900.0);
    assert_eq!(shared.clamp(outside), outside);
    shared.publish(PointerRoute {
        outputs: vec![output(0, 0, 1920, 1080), output(1920, 0, 1280, 720)],
        restrictions: PointerRestrictions::empty(),
    });
    let inside = Point::new(2000.0, 700.0);
    assert_eq!(shared.clamp(inside), inside);
    assert_eq!(shared.clamp(outside), Point::new(2500.0, 719.5));
    assert_eq!(
        shared.clamp(Point::new(-40.0, 500.0)),
        Point::new(0.0, 500.0)
    );
    assert_eq!(
        shared.clamp(Point::new(4000.0, -10.0)),
        Point::new(3199.5, 0.0)
    );
}

#[test]
fn position_keeps_subfixed_precision() {
    let shared = PointerShared::default();
    for warped in [
        Point::new(0.123_456, -0.987_654),
        Point::new(7679.999, 4319.001),
        Point::new(-16383.3, 16383.7),
    ] {
        shared.warp(warped);
        let read = shared.position();
        assert!((read.x - warped.x).abs() <= 1.0 / 1024.0, "{read:?}");
        assert!((read.y - warped.y).abs() <= 1.0 / 1024.0, "{read:?}");
    }
}
