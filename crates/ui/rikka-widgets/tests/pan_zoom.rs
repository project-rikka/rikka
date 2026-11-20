use rikka_types::{Point, Size};
use rikka_widgets::{PanZoom, Zoom};

fn shown() -> PanZoom {
    let mut pan_zoom = PanZoom::new(0.01, 64.0).margin(10.0);
    pan_zoom.show(Size::new(400.0, 200.0));
    pan_zoom.meet(Size::new(220.0, 220.0));
    pan_zoom
}

fn near(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3
}

#[test]
fn fit_leaves_the_margin_and_never_enlarges() {
    let pan_zoom = shown();
    assert_eq!(pan_zoom.scale(), 0.5);
    assert!(near(
        pan_zoom.to_view(Point::new(200.0, 100.0)),
        Point::new(110.0, 110.0)
    ));
    let mut small = PanZoom::new(0.01, 64.0);
    small.show(Size::new(10.0, 10.0));
    small.meet(Size::new(500.0, 500.0));
    assert_eq!(small.scale(), 1.0);
}

#[test]
fn zooming_keeps_the_anchor_under_the_pointer() {
    let anchor = Point::new(30.0, 170.0);
    for scale in [0.25, 3.0, 100.0] {
        let mut pan_zoom = shown();
        let under = pan_zoom.to_content(anchor);
        assert!(pan_zoom.zoom_to(scale, Some(anchor)));
        assert_eq!(pan_zoom.scale(), scale.min(64.0));
        assert!(near(pan_zoom.to_content(anchor), under), "{scale}");
    }
}

#[test]
fn panning_a_fitted_view_keeps_its_scale() {
    let mut pan_zoom = shown();
    assert!(pan_zoom.pan(Point::new(20.0, -10.0)));
    assert_eq!(pan_zoom.zoom(), Zoom::Scale(0.5));
    assert!(near(pan_zoom.center(), Point::new(160.0, 120.0)));
    assert!(pan_zoom.step(&[0.5, 1.0, 2.0], true, None));
    assert_eq!(pan_zoom.scale(), 1.0);
    assert!(pan_zoom.fit());
    assert!(!pan_zoom.fit());
}
