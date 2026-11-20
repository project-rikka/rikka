use crate::support::{at, close, developed, flat, picture};
use rikka_image_edit::{ChromaKey, Clipping, LumaKey, Lumas, Recipe};
use rikka_types::Color;

fn green() -> Color {
    Color::rgb(0.0, 1.0, 0.0)
}

#[test]
fn chroma_key_clears_key_color() {
    let split = picture(8, 2, |x, _| match x < 4 {
        true => [10, 240, 20, 255],
        false => [200, 150, 120, 255],
    });

    let keyed = developed(&split, Recipe::new().then(ChromaKey::new(green())));

    assert_eq!(at(&keyed, 1, 1)[3], 0);
    assert_eq!(at(&keyed, 6, 1)[3], 255);
}

#[test]
fn spill_suppression_removes_key_tint() {
    let tinted = flat(2, 2, [150, 190, 150, 255]);
    let key = ChromaKey::new(green()).tolerance(5.0).softness(0.0);

    let untouched = at(
        &developed(&tinted, Recipe::new().then(key.spill(0.0))),
        0,
        0,
    );
    let cleaned = at(
        &developed(&tinted, Recipe::new().then(key.spill(100.0))),
        0,
        0,
    );

    assert!(
        untouched
            .iter()
            .zip([150, 190, 150, 255])
            .all(|(got, expected)| close(*got, expected, 1)),
        "{untouched:?}"
    );
    assert_eq!(cleaned[3], 255);
    assert!(cleaned[1] < 185 && cleaned[0] > 165, "{cleaned:?}");
    assert!(cleaned[1].abs_diff(cleaned[0]) < 10, "{cleaned:?}");
}

#[test]
fn luma_key_keeps_one_side_of_threshold() {
    let ramp = picture(2, 1, |x, _| match x {
        0 => [20, 20, 20, 255],
        _ => [230, 230, 230, 255],
    });

    let bright = developed(&ramp, Recipe::new().then(LumaKey::new(50.0).softness(0.0)));
    let dark = developed(
        &ramp,
        Recipe::new().then(LumaKey::new(50.0).softness(0.0).keeps(Lumas::Dark)),
    );

    assert_eq!((at(&bright, 0, 0)[3], at(&bright, 1, 0)[3]), (0, 255));
    assert_eq!((at(&dark, 0, 0)[3], at(&dark, 1, 0)[3]), (255, 0));
}

#[test]
fn clipping_clears_edges_and_keeps_size() {
    let white = flat(20, 20, [255, 255, 255, 255]);

    let clipped = developed(
        &white,
        Recipe::new().then(Clipping::new(2.0, 3.0, 4.0, 5.0)),
    );

    assert_eq!(clipped.dimensions(), (20, 20));
    assert_eq!(at(&clipped, 1, 10)[3], 0);
    assert_eq!(at(&clipped, 2, 10)[3], 255);
    assert_eq!(at(&clipped, 15, 10)[3], 255);
    assert_eq!(at(&clipped, 16, 10)[3], 0);
    assert_eq!(at(&clipped, 10, 2)[3], 0);
    assert_eq!(at(&clipped, 10, 3)[3], 255);
    assert_eq!(at(&clipped, 10, 14)[3], 255);
    assert_eq!(at(&clipped, 10, 15)[3], 0);
}
