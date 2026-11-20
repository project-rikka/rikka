use paint_document::{Area, Combine, Modify, Region, Shape};
use rikka_image_edit::FillRule;

const CANVAS: Area = Area::sized(40, 40);

fn spans(region: &Region, y: u32) -> Vec<(u32, u32)> {
    region
        .row(y)
        .iter()
        .map(|span| (span.start, span.end))
        .collect()
}

#[test]
fn subtract_inner_leaves_frame() {
    let outer = Region::rectangle(Area::new(0, 0, 4, 4));
    let inner = Region::rectangle(Area::new(1, 1, 2, 2));

    let frame = outer.combine(Combine::Exclude, inner.clone());

    assert_eq!(spans(&frame, 0), [(0, 4)]);
    assert_eq!(spans(&frame, 1), [(0, 1), (3, 4)]);
    assert_eq!(frame.bounds(), Some(Area::new(0, 0, 4, 4)));
    assert_eq!(frame.rectangular(), None);
    assert_eq!(frame.inverted(Area::sized(4, 4)), inner);
}

#[test]
fn intersect_and_xor_truth_tables() {
    let one = Region::rectangle(Area::new(0, 0, 3, 1));
    let other = Region::rectangle(Area::new(2, 0, 3, 1));

    assert_eq!(
        one.combine(Combine::Intersect, other.clone()).rectangular(),
        Some(Area::new(2, 0, 1, 1))
    );
    assert_eq!(
        spans(&one.combine(Combine::Xor, other.clone()), 0),
        [(0, 2), (3, 5)]
    );
    assert_eq!(one.combine(Combine::Replace, other.clone()), other);
}

#[test]
fn subpixel_rectangle_has_partial_edges() {
    let region = Shape::rectangle(2.5, 2.0, 3.0, 2.0).region(CANVAS);

    assert_eq!(region.coverage(2, 2), 128);
    assert_eq!(region.coverage(3, 2), 255);
    assert_eq!(region.coverage(5, 3), 128);
    assert_eq!(region.coverage(6, 3), 0);
    assert_eq!(region.bounds(), Some(Area::new(2, 2, 4, 2)));
}

#[test]
fn fill_rule_controls_holes() {
    let outer = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
    let inner = vec![(3.0, 3.0), (7.0, 3.0), (7.0, 7.0), (3.0, 7.0)];

    let even_odd = Shape::polygon([outer.clone(), inner.clone()], FillRule::EvenOdd).region(CANVAS);
    let non_zero = Shape::polygon([outer, inner], FillRule::NonZero).region(CANVAS);

    assert_eq!(even_odd.coverage(5, 5), 0);
    assert_eq!(even_odd.coverage(1, 5), 255);
    assert_eq!(non_zero.coverage(5, 5), 255);
}

#[test]
fn shapes_clip_to_canvas_and_ignore_nan() {
    let region = Shape::rectangle(-5.0, -5.0, 100.0, 10.0).region(Area::sized(8, 8));
    let broken = Shape::lasso([(f32::NAN, 1.0), (1.0, 1.0), (5.0, 1.0), (5.0, 5.0)]);

    assert_eq!(region.rectangular(), Some(Area::new(0, 0, 8, 5)));
    assert!(!broken.region(CANVAS).is_empty());
    assert!(
        Shape::ellipse(f32::INFINITY, 0.0, 4.0, 4.0)
            .region(CANVAS)
            .is_empty()
    );
}

#[test]
fn expand_grows_disc() {
    let dot = Region::rectangle(Area::new(10, 10, 1, 1));

    let grown = dot.modified(Modify::Expand(2), CANVAS).expect("expanded");

    assert_eq!(grown.extent(), Some(Area::new(8, 8, 5, 5)));
    assert_eq!(grown.coverage(10, 8), 255);
    assert_eq!(grown.coverage(9, 8), 0);
    assert_eq!(grown.coverage(9, 9), 255);
    assert_eq!(grown.coverage(8, 9), 0);
}

#[test]
fn feather_softens_edge() {
    let soft = Region::rectangle(Area::new(10, 10, 10, 10))
        .modified(Modify::Feather(4.0), CANVAS)
        .expect("feathered");

    assert!(soft.coverage(15, 15) > 240);
    let inside = soft.coverage(10, 15);
    let outside = soft.coverage(9, 15);
    assert!(inside > 128 && inside < 255, "{inside}");
    assert!(outside > 0 && outside < 128, "{outside}");
    assert_eq!(soft.bounds(), Some(Area::new(10, 10, 10, 10)));
    assert!(soft.extent().is_some_and(|extent| extent.width > 10));
}
