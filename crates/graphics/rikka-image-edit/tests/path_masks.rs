use crate::support::{at, developed, flat};
use rikka_image_edit::{FillRule, Invert, Mask, Path, Recipe, Step};
use rikka_types::Point;

fn triangle() -> Path {
    Path::polygon([
        Point::new(2.0, 2.0),
        Point::new(18.0, 2.0),
        Point::new(2.0, 18.0),
    ])
}

fn square(left: f32, top: f32, side: f32) -> [Point; 4] {
    [
        Point::new(left, top),
        Point::new(left + side, top),
        Point::new(left + side, top + side),
        Point::new(left, top + side),
    ]
}

fn inverted_within(mask: Mask) -> rikka_image_edit::Picture {
    developed(
        &flat(20, 20, [0, 0, 0, 255]),
        Recipe::new().then(Step::new(Invert).masked(mask)),
    )
}

#[test]
fn path_mask_confines_step() {
    let masked = inverted_within(Mask::path(triangle(), FillRule::NonZero));

    assert_eq!(at(&masked, 4, 4)[0], 255);
    assert_eq!(at(&masked, 15, 15)[0], 0);
    assert_eq!(at(&masked, 1, 10)[0], 0);
    let slanted = at(&masked, 10, 9)[0];
    assert!(
        slanted > 0 && slanted < 255,
        "the slanted edge is smoothed: {slanted}"
    );
}

#[test]
fn inverted_path_mask_keeps_outside() {
    let masked = inverted_within(Mask::path(triangle(), FillRule::NonZero).inverted());

    assert_eq!(at(&masked, 4, 4)[0], 0);
    assert_eq!(at(&masked, 15, 15)[0], 255);
}

#[test]
fn fill_rule_controls_overlap_holes() {
    let mut builder = Path::builder();
    for corners in [square(2.0, 2.0, 12.0), square(6.0, 6.0, 12.0)] {
        builder = builder.move_to(corners[0]);
        for corner in &corners[1..] {
            builder = builder.line_to(*corner);
        }
        builder = builder.close();
    }
    let overlapping = builder.build();

    let nonzero = inverted_within(Mask::path(overlapping.clone(), FillRule::NonZero));
    let even_odd = inverted_within(Mask::path(overlapping, FillRule::EvenOdd));

    assert_eq!(at(&nonzero, 10, 10)[0], 255);
    assert_eq!(at(&even_odd, 10, 10)[0], 0);
    assert_eq!(at(&even_odd, 3, 3)[0], 255);
    assert_eq!(at(&even_odd, 16, 16)[0], 255);
}

#[test]
fn feathered_path_mask_fades_edge() {
    let hard = inverted_within(Mask::path(
        Path::polygon(square(0.0, 0.0, 10.0)),
        FillRule::NonZero,
    ));
    let soft = inverted_within(
        Mask::path(Path::polygon(square(0.0, 0.0, 10.0)), FillRule::NonZero).feathered(6.0),
    );

    assert_eq!(at(&hard, 9, 5)[0], 255);
    assert_eq!(at(&hard, 10, 5)[0], 0);
    let fading: Vec<u8> = (7..13).map(|x| at(&soft, x, 5)[0]).collect();
    assert!(
        fading.windows(2).all(|pair| pair[0] >= pair[1]),
        "{fading:?}"
    );
    assert!(fading[0] < 255 && fading[5] > 0, "{fading:?}");
}

#[test]
fn luma_matte_confines_step() {
    let matte = crate::support::picture(10, 10, |x, _| match x < 5 {
        true => [255, 255, 255, 255],
        false => [0, 0, 0, 255],
    });

    let masked = inverted_within(Mask::luma(matte));

    assert_eq!(at(&masked, 3, 10)[0], 255);
    assert_eq!(at(&masked, 16, 10)[0], 0);
}

#[test]
fn alpha_matte_follows_coverage() {
    let matte = crate::support::picture(20, 20, |_, y| match y < 10 {
        true => [0, 0, 0, 255],
        false => [0, 0, 0, 0],
    });

    let masked = inverted_within(Mask::alpha(matte));

    assert_eq!(at(&masked, 10, 4)[0], 255);
    assert_eq!(at(&masked, 10, 15)[0], 0);
}
